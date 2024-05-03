import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/features/trade/domain/order.dart';

class AsyncTrade {
  final OrderReason orderReason;

  AsyncTrade({required this.orderReason});

  static AsyncTrade fromApi(bridge.BackgroundTask_AsyncTrade asyncTrade) {
    return AsyncTrade(orderReason: OrderReason.fromApi(asyncTrade.field0));
  }

  static bridge.BackgroundTask apiDummy() {
    return bridge.BackgroundTask_AsyncTrade(OrderReason.apiDummy());
  }
}

enum TaskType { rollover, asyncTrade, collaborativeRevert, fullSync, recover, unknown }

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
}

class BackgroundTask {
  final TaskType type;
  final TaskStatus status;

  String? error;

  BackgroundTask({required this.type, required this.status, this.error});

  static bridge.BackgroundTask apiDummy() {
    return bridge.BackgroundTask_Rollover(TaskStatus.apiDummy());
  }

  static BackgroundTask fromApi(bridge.BackgroundTask task) {
    final taskType = getTaskType(task);

    final (taskStatus, error) = TaskStatus.fromApi(task.field0);
    return BackgroundTask(type: taskType, status: taskStatus, error: error);
  }

  static TaskType getTaskType(bridge.BackgroundTask task) {
    if (task is bridge.BackgroundTask_RecoverDlc) {
      return TaskType.recover;
    }
    if (task is bridge.BackgroundTask_Rollover) {
      return TaskType.rollover;
    }
    if (task is bridge.BackgroundTask_CollabRevert) {
      return TaskType.collaborativeRevert;
    }
    if (task is bridge.BackgroundTask_FullSync) {
      return TaskType.fullSync;
    }

    return TaskType.unknown;
  }
}
