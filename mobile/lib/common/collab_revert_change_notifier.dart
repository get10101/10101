import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/logger/logger.dart';

class CollabRevertChangeNotifier extends ChangeNotifier implements Subscriber {
  TaskStatus taskStatus = TaskStatus.success;

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_BackgroundNotification) {
      if (event.field0 is! bridge.BackgroundTask_CollabRevert) {
        // ignoring other kinds of background tasks
        return;
      }
      CollabRevert collabRevert =
          CollabRevert.fromApi(event.field0 as bridge.BackgroundTask_CollabRevert);
      logger.d("Received a collab revert channel event. Status: ${collabRevert.taskStatus}");

      taskStatus = collabRevert.taskStatus;

      notifyListeners();
    }
  }
}
