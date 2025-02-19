use std::sync::Arc;

use tokio::sync::Mutex;
use zbus::{proxy, zvariant::OwnedObjectPath, Connection};

use super::SyncTarget;

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait SystemdManager {
    async fn get_unit(&self, name: &str) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1"
)]
trait SystemdUnit {}

#[derive(Debug)]
struct Service {
    connection: Arc<Mutex<Connection>>,
    service: SystemdService,
}

#[derive(Debug)]
enum SystemdService {
    Unit(String),
}

impl Service {
    async fn service_path(&self) -> eyre::Result<OwnedObjectPath> {
        let conn = self.connection.lock().await;

        let manager = SystemdManagerProxy::new(&conn).await?;
        let path = match &self.service {
            SystemdService::Unit(name) => manager.get_unit(name).await?,
        };

        Ok(path)
    }
}

impl SyncTarget for Service {
    async fn out_of_sync(&self) -> eyre::Result<bool> {
        let path = self.service_path().await?;

        let conn = self.connection.lock().await;

        match &self.service {
            SystemdService::Unit(_) => {
                let unit = SystemdUnitProxy::new(&conn, path).await?;
            }
        }
    }

    fn sync(self) -> eyre::Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn should_get_unit() {}
}
