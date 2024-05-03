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

enum TaskStatus {
  pending,
  failed,
  success;

  static (TaskStatus, String?) fromApi(bridge.TaskStatus taskStatus) {
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

class Rollover {
  final TaskStatus taskStatus;
  String? error;

  Rollover({required this.taskStatus, this.error});

  static Rollover fromApi(bridge.BackgroundTask_Rollover rollover) {
    final (taskStatus, error) = TaskStatus.fromApi(rollover.field0);
    return Rollover(taskStatus: taskStatus, error: error);
  }

  static bridge.BackgroundTask apiDummy() {
    return bridge.BackgroundTask_Rollover(TaskStatus.apiDummy());
  }
}

class RecoverDlc {
  final TaskStatus taskStatus;
  String? error;

  RecoverDlc({required this.taskStatus, this.error});

  static RecoverDlc fromApi(bridge.BackgroundTask_RecoverDlc recoverDlc) {
    final (taskStatus, error) = TaskStatus.fromApi(recoverDlc.field0);
    return RecoverDlc(taskStatus: taskStatus, error: error);
  }

  static bridge.BackgroundTask apiDummy() {
    return bridge.BackgroundTask_RecoverDlc(TaskStatus.apiDummy());
  }
}

class CollabRevert {
  final TaskStatus taskStatus;
  String? error;

  CollabRevert({required this.taskStatus, this.error});

  static CollabRevert fromApi(bridge.BackgroundTask_CollabRevert collabRevert) {
    final (taskStatus, error) = TaskStatus.fromApi(collabRevert.field0);
    return CollabRevert(taskStatus: taskStatus, error: error);
  }

  static bridge.BackgroundTask apiDummy() {
    return bridge.BackgroundTask_CollabRevert(TaskStatus.apiDummy());
  }
}

class FullSync {
  final TaskStatus taskStatus;
  String? error;

  FullSync({required this.taskStatus, this.error});

  static FullSync fromApi(bridge.BackgroundTask_FullSync fullSync) {
    final (taskStatus, error) = TaskStatus.fromApi(fullSync.field0);
    return FullSync(taskStatus: taskStatus, error: error);
  }

  static bridge.BackgroundTask apiDummy() {
    return bridge.BackgroundTask_FullSync(TaskStatus.apiDummy());
  }
}
