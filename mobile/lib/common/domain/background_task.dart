import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

enum TaskType {
  rollover,
  asyncTrade,
  expire,
  liquidate,
  collaborativeRevert,
  fullSync,
  recover,
  closeChannel,
  unknown
}

enum TaskStatus {
  pending,
  failed,
  success;

  static (TaskStatus, String?) fromApi(dynamic taskStatus) {
    if (taskStatus is bridge.TaskStatus_Pending) {
      return (TaskStatus.pending, null);
    }

    if (taskStatus is bridge.TaskStatus_Success) {
      return (TaskStatus.success, null);
    }

    if (taskStatus is bridge.TaskStatus_Failed) {
      final error = taskStatus.field0;
      return (TaskStatus.failed, error);
    }

    return (TaskStatus.pending, null);
  }

  static bridge.TaskStatus apiDummy() {
    return const bridge.TaskStatus_Pending();
  }

  @override
  String toString() {
    switch (this) {
      case TaskStatus.pending:
        return "Pending";
      case TaskStatus.failed:
        return "Failed";
      case TaskStatus.success:
        return "Success";
    }
  }
}

class BackgroundTask {
  final TaskType type;
  TaskStatus status;

  String? error;

  BackgroundTask({required this.type, required this.status, this.error});

  static bridge.BackgroundTask apiDummy() {
    return bridge.BackgroundTask_Rollover(TaskStatus.apiDummy());
  }

  @override
  String toString() {
    return "$type ($status)";
  }
}
