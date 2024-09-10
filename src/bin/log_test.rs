use log::{error, warn, info, debug, trace};

fn main() {
    env_logger::init();

    trace!("This is a trace log");
    debug!("This is a debug log");
    info!("This is an info log");
    warn!("This is a warning log");
    error!("This is an error log");
}
