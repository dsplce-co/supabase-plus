use std::fmt::{self, Display};

use anyhow::Context;
use cli_table::Cell;
use serde_json::json;
use supercli::output::styling::apply_style;
use tokio_postgres::{Client, Row};

use crate::utils;

const POLICIES: &str = r#"
select
    format('%I.%I', n.nspname, c.relname) as table_name,
    quote_ident(c.relname) as alias,
    pol.polname as name,
    case pol.polcmd
        when 'r' then 'select' when 'a' then 'insert' when 'w' then 'update' when 'd' then 'delete'
        else 'all'
    end as command,
    case
        when not c.relrowsecurity then 'rls off, has no effect'
        when not pol.polpermissive then 'restrictive'
        else ''
    end as note,
    c.relrowsecurity as enabled,
    coalesce(
        case when pol.polcmd = 'a'
            then pg_get_expr(pol.polwithcheck, pol.polrelid)
            else pg_get_expr(pol.polqual, pol.polrelid)
        end,
        'false'
    ) as expression,
    exists (
        select 1 from pg_depend d
        where d.classid = 'pg_policy'::regclass
            and d.objid = pol.oid
            and d.refobjid = pol.polrelid
            and d.refobjsubid > 0
    ) as by_row,
    array(
        select exists (
            select 1 from unnest(pol.polroles) as pr(oid)
            where case when pr.oid = 0 then true else pg_has_role(r.role::name, pr.oid, 'usage') end
        )
        from unnest($4::text[]) with ordinality as r(role, ord) order by r.ord
    ) as applies,
    array(
        select case pol.polcmd
            when 'a' then has_any_column_privilege(r.role, c.oid, 'insert')
            when 'w' then has_any_column_privilege(r.role, c.oid, 'update')
            when 'd' then has_table_privilege(r.role, c.oid, 'delete')
            else has_any_column_privilege(r.role, c.oid, 'select')
        end
        from unnest($4::text[]) with ordinality as r(role, ord) order by r.ord
    ) as granted
from pg_policy pol
join pg_class c on c.oid = pol.polrelid
join pg_namespace n on n.oid = c.relnamespace
where n.nspname = $1
    and ($2::text is null or c.relname = $2)
    and ($3::text is null or pol.polname = $3)
order by table_name, name
"#;

const USER: &str = r#"
select
    coalesce(email, id::text) as label,
    jsonb_build_object(
        'sub', id,
        'aud', 'authenticated',
        'role', 'authenticated',
        'email', email,
        'phone', phone,
        'is_anonymous', is_anonymous,
        'app_metadata', coalesce(raw_app_meta_data, '{}'),
        'user_metadata', coalesce(raw_user_meta_data, '{}')
    )::text as claims
from auth.users
where id::text = lower($1) or lower(email) = lower($1)
"#;

const STRANGER: &str = "00000000-0000-0000-0000-000000000000";

pub struct Identity {
    pub label: String,
    role: &'static str,
    claims: String,
}

impl Identity {
    pub async fn load(client: &Client, user: Option<&str>) -> anyhow::Result<Vec<Self>> {
        let mut identities = vec![
            Self {
                label: "anon".to_string(),
                role: "anon",
                claims: json!({ "role": "anon" }).to_string(),
            },
            Self {
                label: "authenticated".to_string(),
                role: "authenticated",
                claims: json!({ "role": "authenticated", "aud": "authenticated", "sub": STRANGER })
                    .to_string(),
            },
        ];

        if let Some(user) = user {
            let row = client
                .query(USER, &[&user])
                .await
                .context("Couldn't fetch the user")?
                .into_iter()
                .next()
                .with_context(|| {
                    styled_error!("Couldn't find a user with id or email `{}`", (user, "id"))
                })?;

            identities.push(Self {
                label: row.get("label"),
                role: "authenticated",
                claims: row.get("claims"),
            });
        }

        Ok(identities)
    }
}

pub enum Verdict {
    NotApplicable,
    NoGrant,
    Rows { passing: i64, total: i64 },
    Allows(bool),
    ByRow,
    Failed(String),
}

impl Verdict {
    fn cell(&self) -> (String, &'static str) {
        match self {
            Self::NotApplicable => ("-".to_string(), "muted"),
            Self::NoGrant => ("no grant".to_string(), "warning"),
            Self::Rows { passing: 0, total } => (format!("0/{total}"), "muted"),
            Self::Rows { passing, total } => (format!("{passing}/{total}"), "success"),
            Self::Allows(true) => ("yes".to_string(), "success"),
            Self::Allows(false) => ("no".to_string(), "muted"),
            Self::ByRow => ("by row".to_string(), "muted"),
            Self::Failed(_) => ("error".to_string(), "warning"),
        }
    }
}

pub struct PolicyRow {
    pub table: String,
    pub name: String,
    command: String,
    note: String,
    enabled: bool,
    alias: String,
    expression: String,
    by_row: bool,
    applies: Vec<bool>,
    granted: Vec<bool>,
    pub verdicts: Vec<Verdict>,
}

impl From<Row> for PolicyRow {
    fn from(row: Row) -> Self {
        Self {
            table: row.get("table_name"),
            name: row.get("name"),
            command: row.get("command"),
            note: row.get("note"),
            enabled: row.get("enabled"),
            alias: row.get("alias"),
            expression: row.get("expression"),
            by_row: row.get("by_row"),
            applies: row.get("applies"),
            granted: row.get("granted"),
            verdicts: Vec::new(),
        }
    }
}

impl PolicyRow {
    async fn evaluate(
        &self,
        client: &mut Client,
        identity: &Identity,
    ) -> Result<Verdict, tokio_postgres::Error> {
        let checks_new_row = self.command == "insert";

        if checks_new_row && self.by_row {
            return Ok(Verdict::ByRow);
        }

        let transaction = client.transaction().await?;

        transaction
            .batch_execute(&format!(
                "create temp view sbp_rows as select * from {}; grant select on pg_temp.sbp_rows to {}",
                self.table, identity.role
            ))
            .await?;

        transaction
            .query_one(
                "select set_config('role', $1, true), set_config('request.jwt.claims', $2, true), set_config('transaction_read_only', 'on', true), set_config('statement_timeout', '30s', true)",
                &[&identity.role, &identity.claims],
            )
            .await?;

        if checks_new_row {
            let row = transaction
                .query_one(&format!("select ({})", self.expression), &[])
                .await?;

            return Ok(Verdict::Allows(row.get::<_, Option<bool>>(0) == Some(true)));
        }

        let row = transaction
            .query_one(
                &format!(
                    "select count(*) filter (where ({})), count(*) from pg_temp.sbp_rows as {}",
                    self.expression, self.alias
                ),
                &[],
            )
            .await?;

        Ok(Verdict::Rows {
            passing: row.get(0),
            total: row.get(1),
        })
    }

    async fn judge(&mut self, client: &mut Client, identities: &[Identity]) {
        for (index, identity) in identities.iter().enumerate() {
            let verdict = match (self.applies[index], self.granted[index]) {
                (false, _) => Verdict::NotApplicable,
                (_, false) => Verdict::NoGrant,
                _ => self
                    .evaluate(client, identity)
                    .await
                    .unwrap_or_else(|error| {
                        Verdict::Failed(
                            error
                                .as_db_error()
                                .map_or_else(|| error.to_string(), |db| db.message().to_string()),
                        )
                    }),
            };

            self.verdicts.push(verdict);
        }
    }
}

pub struct PolicyReport {
    pub identities: Vec<Identity>,
    pub rows: Vec<PolicyRow>,
}

impl PolicyReport {
    pub async fn load(
        client: &mut Client,
        schema: &str,
        table: Option<&str>,
        name: Option<&str>,
        identities: Vec<Identity>,
    ) -> anyhow::Result<Self> {
        let roles: Vec<&str> = identities.iter().map(|identity| identity.role).collect();

        let mut rows: Vec<PolicyRow> = client
            .query(POLICIES, &[&schema, &table, &name, &roles])
            .await
            .with_context(|| {
                styled_error!("Couldn't fetch policies for `{}` schema", (schema, "id"))
            })?
            .into_iter()
            .map(PolicyRow::from)
            .collect();

        for row in &mut rows {
            row.judge(client, &identities).await;
        }

        Ok(Self { identities, rows })
    }

    pub fn failures(&self) -> impl Iterator<Item = (&PolicyRow, &str, &str)> {
        self.rows.iter().flat_map(|row| {
            self.identities
                .iter()
                .zip(&row.verdicts)
                .filter_map(move |(identity, verdict)| match verdict {
                    Verdict::Failed(message) => {
                        Some((row, identity.label.as_str(), message.as_str()))
                    }
                    _ => None,
                })
        })
    }
}

impl Display for PolicyReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = ["table", "policy", "command"]
            .into_iter()
            .chain(
                self.identities
                    .iter()
                    .map(|identity| identity.label.as_str()),
            )
            .chain(["note"])
            .map(|title| apply_style(title, "muted").cell());

        let rows = self.rows.iter().map(|row| {
            let verdicts = row.verdicts.iter().map(|verdict| {
                let (text, style) = verdict.cell();

                apply_style(text, style).cell()
            });

            let style = if row.enabled { "muted" } else { "warning" };

            [
                row.table.as_str().cell(),
                row.name.as_str().cell(),
                row.command.as_str().cell(),
            ]
            .into_iter()
            .chain(verdicts)
            .chain([apply_style(&row.note, style).cell()])
            .collect::<Vec<_>>()
        });

        let table = utils::table(header.collect(), rows);

        writeln!(f, "{table}")?;

        let counts: Vec<(i64, i64)> = self
            .rows
            .iter()
            .flat_map(|row| &row.verdicts)
            .filter_map(|verdict| match verdict {
                Verdict::Rows { passing, total } => Some((*passing, *total)),
                _ => None,
            })
            .collect();

        let sample = counts
            .iter()
            .find(|(passing, total)| *passing > 0 && passing < total)
            .or(counts.first());

        let mut legend = Vec::new();

        if let Some((passing, total)) = sample {
            legend.push(format!(
                "{:<8} the policy lets through {passing} of {total} rows in the table",
                format!("{passing}/{total}")
            ));
        }

        let has =
            |check: fn(&Verdict) -> bool| self.rows.iter().flat_map(|row| &row.verdicts).any(check);

        if has(|verdict| matches!(verdict, Verdict::Allows(_))) {
            legend
                .push("yes/no   insert check that doesn't depend on the inserted row".to_string());
        }

        if has(|verdict| matches!(verdict, Verdict::ByRow)) {
            legend.push(
                "by row   insert check that depends on the inserted row, so it can't be evaluated without one"
                    .to_string(),
            );
        }

        if has(|verdict| matches!(verdict, Verdict::NotApplicable)) {
            legend.push("-        the policy isn't for this role".to_string());
        }

        if has(|verdict| matches!(verdict, Verdict::NoGrant)) {
            legend.push("no grant the role has no privilege for this command".to_string());
        }

        if has(|verdict| matches!(verdict, Verdict::Failed(_))) {
            legend.push("error    the policy couldn't be evaluated, details below".to_string());
        }

        write!(f, "\n{}", apply_style(legend.join("\n"), "muted"))
    }
}
