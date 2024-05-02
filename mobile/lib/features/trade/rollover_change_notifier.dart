import 'package:get_10101/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';

class RolloverChangeNotifier extends ChangeNotifier implements Subscriber {
  TaskStatus taskStatus = TaskStatus.success;

  @override
  void notify(bridge.Event event) async {
    if (event is bridge.Event_BackgroundNotification) {
      if (event.field0 is! bridge.BackgroundTask_Rollover) {
        // ignoring other kinds of background tasks
        return;
      }

      Rollover rollover = Rollover.fromApi(event.field0 as bridge.BackgroundTask_Rollover);
      logger.d("Received a rollover event. Status: ${rollover.taskStatus}");

      taskStatus = rollover.taskStatus;

      notifyListeners();
    } else {
      logger.w("Received unexpected event: ${event.toString()}");
    }
  }
}
