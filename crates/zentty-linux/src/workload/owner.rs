//! Invoked by the user manager, not a GTK callback. The inherited descriptor
//! refers to a kernel process identity, so PID reuse cannot extend its lease.
pub fn wait_for_gui_exit() -> Result<(), String> {
    let info = std::fs::read_to_string("/proc/self/fdinfo/0")
        .map_err(|error| format!("cannot inspect owner pidfd: {error}"))?;
    if !info.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(key, value)| key == "Pid" && value.trim().parse::<i32>().is_ok())
    }) {
        return Err("workload owner requires a pidfd on standard input".into());
    }
    let input = std::io::stdin();
    let mut descriptors = [rustix::event::PollFd::new(
        &input,
        rustix::event::PollFlags::IN,
    )];
    loop {
        match rustix::event::poll(&mut descriptors, None) {
            Ok(_)
                if descriptors[0]
                    .revents()
                    .contains(rustix::event::PollFlags::IN) =>
            {
                return Ok(());
            }
            Ok(_) => return Err("workload owner pidfd became invalid".into()),
            Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(format!("workload owner wait failed: {error}")),
        }
    }
}
