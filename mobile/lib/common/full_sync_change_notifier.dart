import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/logger/logger.dart';

class FullSyncChangeNotifier extends ChangeNotifier implements Subscriber {
  TaskStatus taskStatus = TaskStatus.success;

  @override
  void notify(bridge.Event event) async {
    if (event is bridge.Event_BackgroundNotification) {
      if (event.field0 is! bridge.BackgroundTask_FullSync) {
        // ignoring other kinds of background tasks
        return;
      }
      FullSync fullSync = FullSync.fromApi(event.field0 as bridge.BackgroundTask_FullSync);
      logger.d("Received a full sync event. Status: ${fullSync.taskStatus}");

      taskStatus = fullSync.taskStatus;

      notifyListeners();
    }
  }
}
