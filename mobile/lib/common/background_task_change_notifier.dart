import 'package:flutter/material.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/logger/logger.dart';

class Stack<E> {
  final _list = <E>[];

  void push(E value) => _list.add(value);

  E pop() => _list.removeLast();

  E get peek => _list.last;

  bool get isEmpty => _list.isEmpty;

  bool get isNotEmpty => _list.isNotEmpty;

  @override
  String toString() => _list.toString();
}

class BackgroundTaskChangeNotifier extends ChangeNotifier implements Subscriber {
  Stack<BackgroundTask> events = Stack();

  @override
  void notify(bridge.Event event) async {
    if (event is bridge.Event_BackgroundNotification) {
      logger.d("Received a background task notification. ${event.field0}");
      final (taskStatus, error) = TaskStatus.fromApi(event.field0.field0);
      if (event.field0 is bridge.BackgroundTask_RecoverDlc) {
        events.push(BackgroundTask(type: TaskType.recover, status: taskStatus, error: error));
        notifyListeners();
      }

      if (event.field0 is bridge.BackgroundTask_FullSync) {
        events.push(BackgroundTask(type: TaskType.fullSync, status: taskStatus, error: error));
        notifyListeners();
      }

      if (event.field0 is bridge.BackgroundTask_Rollover) {
        events.push(BackgroundTask(type: TaskType.rollover, status: taskStatus, error: error));
        notifyListeners();
      }

      if (event.field0 is bridge.BackgroundTask_CollabRevert) {
        events.push(
            BackgroundTask(type: TaskType.collaborativeRevert, status: taskStatus, error: error));
        notifyListeners();
      }

      if (event.field0 is bridge.BackgroundTask_AsyncTrade) {
        events.push(BackgroundTask(type: TaskType.asyncTrade, status: taskStatus, error: error));
        notifyListeners();
      }
    }
  }
}
