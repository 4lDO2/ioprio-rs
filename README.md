# `ioprio-rs`

A crate for managing Linux I/O priorities, either globally for one or more
processes, or in advanced interfaces such as io_uring and Linux AIO. It allows
setting the `ioprio` field of io_uring SQE:s directly when the `iou` Cargo
feature is enabled.
