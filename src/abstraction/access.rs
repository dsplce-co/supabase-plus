use std::fmt::{self, Display};

use anyhow::Context;
use supercli::output::styling::apply_style;
use tokio_postgres::Row;

use crate::{abstraction::SupabaseProject, errors::NoWay};

const TABLES: &str = r#"
select
    case when c.relkind in ('v', 'm') then 'view' else 'table' end as kind,
    format('%I.%I', n.nspname, c.relname) as name,
    case
        when c.relkind in ('v', 'm') and g.guarded then 'security invoker'
        when c.relkind in ('v', 'm') then 'runs as owner'
        when g.guarded then 'rls on'
        else 'rls off'
    end as note,
    g.guarded,
    array(
        select concat_ws('',
            case when has_table_privilege(r.role, c.oid, 'select') then 'S'
                when has_any_column_privilege(r.role, c.oid, 'select') then 's' end,
            case when has_table_privilege(r.role, c.oid, 'insert') then 'I'
                when has_any_column_privilege(r.role, c.oid, 'insert') then 'i' end,
            case when has_table_privilege(r.role, c.oid, 'update') then 'U'
                when has_any_column_privilege(r.role, c.oid, 'update') then 'u' end,
            case when has_table_privilege(r.role, c.oid, 'delete') then 'D' end
        )
        from unnest($2::text[]) with ordinality as r(role, ord) order by r.ord
    ) as held
from pg_class c
join pg_namespace n on n.oid = c.relnamespace
cross join lateral (
    select case
        when c.relkind in ('v', 'm') then coalesce(
            (select option_value::bool from pg_options_to_table(c.reloptions) where option_name = 'security_invoker'),
            false
        )
        else c.relrowsecurity
    end as guarded
) g
where n.nspname = $1
    and c.relkind in ('r', 'p', 'v', 'm', 'f')
    and not exists (select 1 from pg_depend d where d.objid = c.oid and d.deptype = 'e')
order by kind, name
"#;

const SEQUENCES: &str = r#"
select
    'sequence' as kind,
    format('%I.%I', n.nspname, c.relname) as name,
    '' as note,
    coalesce(
        (
            select owner.relrowsecurity
            from pg_depend d
            join pg_class owner on owner.oid = d.refobjid
            where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype = 'a'
        ),
        true
    ) as guarded,
    array(
        select case when has_sequence_privilege(r.role, c.oid, 'usage') then 'U' else '' end
        from unnest($2::text[]) with ordinality as r(role, ord) order by r.ord
    ) as held
from pg_class c
join pg_namespace n on n.oid = c.relnamespace
where n.nspname = $1
    and c.relkind = 'S'
    and not exists (
        select 1 from pg_depend d
        where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype in ('e', 'i')
    )
order by name
"#;

const FUNCTIONS: &str = r#"
select
    'function' as kind,
    format('%I.%I(%s)', n.nspname, p.proname, pg_get_function_identity_arguments(p.oid)) as name,
    case when p.prosecdef then 'security definer' else '' end as note,
    not p.prosecdef as guarded,
    array(
        select case when has_function_privilege(r.role, p.oid, 'execute') then 'X' else '' end
        from unnest($2::text[]) with ordinality as r(role, ord) order by r.ord
    ) as held
from pg_proc p
join pg_namespace n on n.oid = p.pronamespace
where n.nspname = $1
    and p.prokind = 'f'
    and not exists (select 1 from pg_depend d where d.objid = p.oid and d.deptype = 'e')
order by name
"#;

const COLUMNS: &str = r#"
select kind, name, note, guarded, held
from (
    select
        'column' as kind,
        format('%I.%I.%I', n.nspname, c.relname, a.attname) as name,
        '' as note,
        false as guarded,
        array(
            select concat_ws('',
                case when has_column_privilege(r.role, c.oid, a.attnum, 'select')
                    and not has_table_privilege(r.role, c.oid, 'select') then 'S' end,
                case when has_column_privilege(r.role, c.oid, a.attnum, 'insert')
                    and not has_table_privilege(r.role, c.oid, 'insert') then 'I' end,
                case when has_column_privilege(r.role, c.oid, a.attnum, 'update')
                    and not has_table_privilege(r.role, c.oid, 'update') then 'U' end
            )
            from unnest($2::text[]) with ordinality as r(role, ord) order by r.ord
        ) as held
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    join pg_attribute a on a.attrelid = c.oid
    where n.nspname = $1
        and c.relkind in ('r', 'p', 'v', 'm', 'f')
        and a.attnum > 0
        and not a.attisdropped
        and not exists (select 1 from pg_depend d where d.objid = c.oid and d.deptype = 'e')
) columns
where array_to_string(held, '') <> ''
order by name
"#;

const AUTO_GRANTING_QUERY: &str = r#"
select exists (
    select 1
    from pg_default_acl d, aclexplode(d.defaclacl) a
    where d.defaclrole = 'postgres'::regrole
        and d.defaclnamespace = (select oid from pg_namespace where nspname = $1)
        and d.defaclobjtype = 'r'
        and a.privilege_type = 'SELECT'
        and a.grantee in (select oid from pg_roles where rolname = any($2))
)
"#;

#[derive(Clone, Copy, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub enum ObjectKind {
    Table,
    View,
    Sequence,
    Function,
    Column,
}

impl ObjectKind {
    fn parse(kind: &str) -> Option<Self> {
        match kind {
            "table" => Some(Self::Table),
            "view" => Some(Self::View),
            "sequence" => Some(Self::Sequence),
            "function" => Some(Self::Function),
            "column" => Some(Self::Column),
            _ => None,
        }
    }

    pub fn recommended(self, role: &str) -> Option<&'static str> {
        match (self, role) {
            (Self::Table, "anon") | (Self::View, _) => Some("select"),
            (Self::Table, _) => Some("select, insert, update, delete"),
            (Self::Sequence, "anon") => None,
            (Self::Sequence, _) => Some("usage"),
            (Self::Function, _) => Some("execute"),
            (Self::Column, _) => None,
        }
    }

    pub fn sql_object(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Sequence => "sequence",
            _ => "table",
        }
    }
}

pub struct AccessRow {
    pub kind: ObjectKind,
    pub name: String,
    pub note: String,
    pub guarded: bool,
    pub held: Vec<String>,
}

impl AccessRow {
    pub fn missing(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        let untouched = self.held.iter().all(String::is_empty);

        AccessMatrix::ROLES.into_iter().filter_map(move |role| {
            let privileges = self.kind.recommended(role).filter(|_| untouched)?;

            Some((role, privileges))
        })
    }
}

impl From<Row> for AccessRow {
    fn from(row: Row) -> Self {
        Self {
            kind: ObjectKind::parse(row.get("kind"))
                .no_way_because("the query selects only known kinds"),
            name: row.get("name"),
            note: row.get("note"),
            guarded: row.get("guarded"),
            held: row.get("held"),
        }
    }
}

pub struct AccessMatrix {
    pub rows: Vec<AccessRow>,
    pub auto_granting: bool,
}

impl AccessMatrix {
    pub const ROLES: [&'static str; 3] = ["anon", "authenticated", "service_role"];

    const QUERIES: [&'static str; 4] = [TABLES, SEQUENCES, FUNCTIONS, COLUMNS];

    pub async fn load(project: &SupabaseProject, schema: &str) -> anyhow::Result<Self> {
        let roles = Self::ROLES.as_slice();
        let mut rows = Vec::new();

        for query in Self::QUERIES {
            let fetched = project
                .runtime()
                .query(query, &[&schema, &roles])
                .await
                .with_context(|| {
                    styled_error!("Couldn't fetch access for `{}` schema", (schema, "id"))
                })?;

            rows.extend(fetched.into_iter().map(AccessRow::from));
        }

        let auto_granting = project
            .runtime()
            .query(AUTO_GRANTING_QUERY, &[&schema, &roles])
            .await
            .with_context(|| {
                styled_error!(
                    "Couldn't fetch default privileges for `{}` schema",
                    (schema, "id")
                )
            })?
            .iter()
            .any(|row| row.get(0));

        Ok(Self {
            rows,
            auto_granting,
        })
    }

    fn gaps(&self) -> impl Iterator<Item = &AccessRow> {
        self.rows
            .iter()
            .filter(|row| row.missing().next().is_some())
    }

    pub fn fixable(&self) -> impl Iterator<Item = &AccessRow> {
        self.gaps().filter(|row| row.guarded)
    }

    pub fn left_out(&self) -> impl Iterator<Item = &AccessRow> {
        self.gaps().filter(|row| !row.guarded)
    }
}

impl Display for AccessMatrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name_width = self
            .rows
            .iter()
            .map(|row| row.name.len())
            .max()
            .unwrap_or(0);

        let header = format!("{:<8} {:<name_width$} {}", "", "", Self::ROLES.join(" "));
        writeln!(f, "{}", apply_style(header, "muted"))?;

        for row in &self.rows {
            let mut line = format!("{:<8} {:<name_width$}", row.kind.to_string(), row.name);

            for (role, held) in Self::ROLES.iter().zip(&row.held) {
                let cell = format!(
                    "{:<width$}",
                    if held.is_empty() { "-" } else { held },
                    width = role.len()
                );

                let style = if held.is_empty() { "muted" } else { "success" };

                line.push_str(&format!(" {}", apply_style(cell, style)));
            }

            let style = if row.guarded { "muted" } else { "warning" };

            writeln!(f, "{line} {}", apply_style(&row.note, style))?;
        }

        write!(
            f,
            "\n{}",
            apply_style(
                "S select  I insert  U update or usage  D delete  X execute  (lowercase: some columns only)",
                "muted"
            )
        )
    }
}
