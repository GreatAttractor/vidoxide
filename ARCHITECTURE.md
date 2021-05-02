# Vidoxide architecture


## 1. Program data

Vidoxide keeps its state in a single instance of `ProgramData` struct, wrapped in `Rc<RefCell>`. Many event handler closures take a clone of this `Rc`.


## 2. Configuration

Persistent configuration is saved in an INI file in the OS-specific user configuration directory (see `config.rs`).


## 3. Threading

Vidoxide uses 5 threads:

|  | Purpose    | Lifetime                          |
|--|------------|-----------------------------------|
|T1| Main (GUI) | program execution duration        |
|T2| Timer      | program execution duration        |
|T3| Capture    | from camera connect to disconnect |
|T4| Recording  | program execution duration        |
|T5| Histogram  | program execution duration        |


GTK and its supporting libraries' functions are called only in T1, with the exception of message passing via `glib::Sender` in other threads which communicate with T1.

T2's only role is sending notifications to T1 in 1-second intervals (upon notification T1 updates the status bar, refreshes the readable camera controls, etc.).

T3 is created upon connecting to a camera, and constantly captures frames. Frames are sent to T1 for preview and to T4 for recording (if a recording job has been started). T3 also performs image feature tracking (if enabled).

T4 records frames sent by T3, if recording is in progress; otherwise, it waits for a new recording job.

T5 determines image histogram when requested by T1.


### 3.1. Thread communication

Threads communicate mainly by message passing. Possible messages are defined in the enums `MainToCaptureThreadMsg`, `RecordingToMainThreadMsg` etc.

Additionally:
  - T3 makes a decision whether to allocate a new (`Arc`-wrapped) capture buffer by checking if it has exclusive ownership of one of the previous buffers
  - T1 uses `new_preview_wanted: Arc<AtomicBool>` to let T3 now it's ready for a new preview image
  - T1, T3 and T4 use `buffered_kib: Arc<AtomicIsize>` containing the amount of data captured, but not yet recorded (if recording is in progress); the value is used to limit the buffered amount and for user information, so exact/fully synchronized updates are not required


## 4. Video capture

A camera driver defines a `camera::Camera`-implementing struct (which is stored in `ProgramData` in T1) and a `camera::FrameCapturer`-implementing struct (which is kept by T3). For the currently implemented drivers, a camera and its frame capturer hold a shared handle, which can be operated on from multiple threads without additional synchronization. E.g., for IIDC it is a `dc1394camera_t`, for FlyCapture2: an `fc2Context`, and for V4L2: a `std::os::unix::io::RawFd`. If some new driver requires a different approach, this architecture may need to be updated (e.g., by adding an optional T1/T3 synchronization mechanism).