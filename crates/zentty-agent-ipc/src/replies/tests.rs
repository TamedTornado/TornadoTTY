use super::*;
use std::fmt::Write as _;
use std::io::Read;

#[test]
fn reservations_bound_live_and_queued_replies_and_release_on_drop() {
    let (queue, _receiver) = ReplyQueue::new();
    let mut held = Vec::new();
    for pane in 0..4 {
        for _ in 0..PER_PANE_CAPACITY {
            held.push(queue.reserve(&format!("pane-{pane}")).unwrap());
        }
        assert!(queue.reserve(&format!("pane-{pane}")).is_err());
    }
    assert!(queue.reserve("quiet").is_err());
    held.pop();
    assert!(queue.reserve("quiet").is_ok());
    drop(held);
    let slots = queue.occupied.lock().unwrap();
    assert_eq!(slots.total, 0);
    assert!(slots.by_pane.is_empty());
}

#[test]
fn saturated_peer_does_not_block_other_replies_and_partial_writes_preserve_bytes() {
    let (queue, receiver) = ReplyQueue::new();
    let (mut stream, mut peer) = UnixStream::pair().unwrap();
    stream.set_nonblocking(true).unwrap();
    peer.set_nonblocking(true).unwrap();
    // Fill the real kernel send buffer to force WouldBlock regardless of its
    // configured size. The prefix is removed before checking the actual reply.
    let mut prefix = 0;
    loop {
        match stream.write(&[b'p'; 4096]) {
            Ok(count) => prefix += count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            result => panic!("unexpected socket fill result: {result:?}"),
        }
        assert!(prefix < 4 * 1024 * 1024);
    }
    let (sender, response) = mpsc::sync_channel(1);
    let mut text = String::new();
    for i in 0..27_000 {
        write!(text, "{i:08x};").unwrap();
    }
    sender
        .send(TmuxCompatReply::success(text.clone()).unwrap())
        .unwrap();
    queue.enqueue(
        stream,
        PendingReply::new(
            "large".to_owned(),
            ReplyReceiver::Tmux(response),
            queue.reserve("busy").unwrap(),
        ),
    );
    let mut busy = receiver.recv().unwrap();
    assert!(!busy.advance(Instant::now()));
    assert_eq!(busy.written, 0);

    let (quiet_stream, mut quiet_peer) = UnixStream::pair().unwrap();
    let (sender, response) = mpsc::sync_channel(1);
    sender
        .send(TmuxCompatReply::success("quiet".to_owned()).unwrap())
        .unwrap();
    queue.enqueue(
        quiet_stream,
        PendingReply::new(
            "quiet".to_owned(),
            ReplyReceiver::Tmux(response),
            queue.reserve("quiet").unwrap(),
        ),
    );
    let mut quiet = receiver.recv().unwrap();
    assert!(quiet.advance(Instant::now()));
    drop(quiet);
    quiet_peer
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut quiet_bytes = Vec::new();
    quiet_peer.read_to_end(&mut quiet_bytes).unwrap();
    let quiet_reply: serde_json::Value = serde_json::from_slice(&quiet_bytes).unwrap();
    assert_eq!(quiet_reply["result"]["stdout"], "quiet");

    let mut bytes_seen = Vec::new();
    let mut buffer = [0; 8192];
    let mut finished = false;
    for _ in 0..1000 {
        match peer.read(&mut buffer) {
            Ok(count) => bytes_seen.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            result => panic!("unexpected socket read: {result:?}"),
        }
        if !finished {
            finished = busy.advance(Instant::now());
        }
        if finished && bytes_seen.len() == prefix + busy.bytes.as_ref().unwrap().len() {
            break;
        }
    }
    assert!(finished);
    assert_eq!(&bytes_seen[prefix..], busy.bytes.as_ref().unwrap());
    let response: serde_json::Value = serde_json::from_slice(&bytes_seen[prefix..]).unwrap();
    assert_eq!(response["id"], "large");
    assert_eq!(response["result"]["stdout"], text);
    drop(busy);
    assert_eq!(queue.occupied.lock().unwrap().total, 0);
}

#[test]
fn timeout_and_receiver_disconnect_return_errors_without_waiting() {
    for disconnected in [false, true] {
        let (queue, receiver) = ReplyQueue::new();
        let (stream, mut peer) = UnixStream::pair().unwrap();
        let (sender, response) = mpsc::sync_channel(1);
        let mut pending = PendingReply::new(
            "late".to_owned(),
            ReplyReceiver::Tmux(response),
            queue.reserve("pane").unwrap(),
        );
        let now = Instant::now();
        if disconnected {
            drop(sender);
        } else {
            pending.deadline = now;
        }
        queue.enqueue(stream, pending);
        let mut connection = receiver.recv().unwrap();
        assert!(connection.advance(now));
        drop(connection);
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let mut bytes = Vec::new();
        peer.read_to_end(&mut bytes).unwrap();
        let response: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(response["ok"], false);
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("response timed out")
        );
        assert_eq!(queue.occupied.lock().unwrap().total, 0);
    }
}

#[test]
fn live_reply_waits_until_ready_and_a_closed_peer_releases_it_immediately() {
    let (queue, receiver) = ReplyQueue::new();
    let (stream, peer) = UnixStream::pair().unwrap();
    let (sender, response) = mpsc::sync_channel(1);
    queue.enqueue(
        stream,
        PendingReply::new(
            "pending".to_owned(),
            ReplyReceiver::Tmux(response),
            queue.reserve("pane").unwrap(),
        ),
    );
    let mut connection = receiver.recv().unwrap();
    assert!(
        !connection.advance(Instant::now()),
        "a live unanswered GUI request must not fail early"
    );
    assert!(connection.bytes.is_none());
    drop(peer);
    sender
        .send(TmuxCompatReply::success("done".to_owned()).unwrap())
        .unwrap();
    assert!(
        connection.advance(Instant::now()),
        "a dead socket must not occupy its slot until timeout"
    );
    drop(connection);
    assert_eq!(queue.occupied.lock().unwrap().total, 0);
}
