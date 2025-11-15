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

    Ok(docker
        .list_containers(None::<ListContainersOptions>)
        .await
        .unwrap()
        .into_iter()
        .find(|container| {
            let has_random_name = container
                .names
                .as_ref()
                .map(|indeed_names| {
                    indeed_names
                        .iter()
                        .any(|name| !name.starts_with("/supabase_"))
                })
                .unwrap_or_default();

            if !has_random_name {
                return false;
            }

            let Some(ports) = container.ports.as_ref() else {
                return false;
            };

            let mut is_54320 = false;

            for port in ports {
                if port.public_port == Some(54320) {
                    is_54320 = true;
                    break;
                }
            }

            if !is_54320 {
                return false;
            }

            container
                .image
                .as_ref()
                .map_or(false, |image| image.contains("supabase/postgres"))
        }))
}
