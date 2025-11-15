use anyhow::Context;
use bollard::{Docker, query_parameters::ListContainersOptions, secret::ContainerSummary};

lazy_static::lazy_static! {
    pub static ref NO_DOCKER: String = crate::styled_error!(
        "It seems that either you don't have Docker installed or its socket/pipe file is broken/not available"
    );
}

pub async fn supabase() -> anyhow::Result<Vec<ContainerSummary>> {
    let docker = Docker::connect_with_socket_defaults().with_context(|| NO_DOCKER.clone())?;

    Ok(docker
        .list_containers(None::<ListContainersOptions>)
        .await
        .unwrap()
        .into_iter()
        .filter(|container| {
            container
                .names
                .as_ref()
                .map(|indeed_names| {
                    indeed_names
                        .iter()
                        .any(|name| name.starts_with("/supabase_"))
                })
                .unwrap_or_default()
        })
        .collect())
}

pub async fn shadow_db() -> anyhow::Result<Option<ContainerSummary>> {
    let docker = Docker::connect_with_socket_defaults().with_context(|| NO_DOCKER.clone())?;

    let containers = docker
        .list_containers(None::<ListContainersOptions>)
        .await
        .unwrap();

    for item in containers.into_iter() {
        let has_random_name = item
            .names
            .as_ref()
            .map(|indeed_names| {
                indeed_names
                    .iter()
                    .any(|name| !name.starts_with("/supabase_"))
            })
            .unwrap_or_default();

        if !has_random_name {
            continue;
        }

        let Some(ports) = item.ports.as_ref() else {
            continue;
        };

        let is_54320 = ports.iter().any(|item| item.public_port == Some(54320));

        if !is_54320 {
            continue;
        }

        let Some(image) = item.image.as_ref() else {
            continue;
        };

        if image.contains("supabase/postgres") {
            return Ok(Some(item));
        }
    }

    Ok(None)
}
