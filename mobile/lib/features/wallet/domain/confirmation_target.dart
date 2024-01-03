import 'package:get_10101/ffi.dart' as rust;

enum ConfirmationTarget {
  minimum,
  background,
  normal,
  highPriority;

  static ConfirmationTarget fromAPI(rust.ConfirmationTarget target) {
    return switch (target) {
      rust.ConfirmationTarget.Background => background,
      rust.ConfirmationTarget.Normal => normal,
      rust.ConfirmationTarget.HighPriority => highPriority,
      rust.ConfirmationTarget.Minimum => minimum,
    };
  }

  static const List<ConfirmationTarget> options = [
    ConfirmationTarget.background,
    ConfirmationTarget.normal,
    ConfirmationTarget.highPriority,
  ];

  @override
  String toString() {
    return switch (this) {
      minimum => "Minimum",
      background => "Background",
      normal => "Normal",
      highPriority => "High Priority",
    };
  }

  // TODO(restioson): are these correct?
  String toTimeEstimate() {
    return switch (this) {
      ConfirmationTarget.minimum => "eventually",
      // LDK says 'within the next day or so'
      ConfirmationTarget.background => "~1 day",
      // LDK says 'within the next 12-24 blocks'
      ConfirmationTarget.normal => "~2-4 hours",
      // LDK says 'within the next few blocks'
      ConfirmationTarget.highPriority => "~10-30 min",
    };
  }

  rust.ConfirmationTarget toAPI() {
    return switch (this) {
      minimum => rust.ConfirmationTarget.Minimum,
      background => rust.ConfirmationTarget.Background,
      normal => rust.ConfirmationTarget.Normal,
      highPriority => rust.ConfirmationTarget.HighPriority,
    };
  }
}
