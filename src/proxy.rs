mod connect;
mod http;
mod request;
mod response;
mod upgrade;
mod upstream;

use std::time::SystemTime;

pub(crate) use connect::handle;

fn now_ts() -> String {
    format!(
        "{:?}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
    )
}
