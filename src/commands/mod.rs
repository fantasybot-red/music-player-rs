mod cdev;
mod cloop;
mod cnowplaying;
mod cplay;
mod cqueue;
mod cskip;
mod cstop;
mod cvolume;

pub use cdev::dev;
pub use cloop::loop_cmd;
pub use cnowplaying::nowplaying;
pub use cplay::play;
pub use cqueue::queue;
pub use cskip::skip;
pub use cstop::stop;
pub use cvolume::volume;
