import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/logger/logger.dart';

class RecoverDlcChangeNotifier extends ChangeNotifier implements Subscriber {
  TaskStatus taskStatus = TaskStatus.success;

  @override
  void notify(bridge.Event event) async {
    if (event is bridge.Event_BackgroundNotification) {
      if (event.field0 is! bridge.BackgroundTask_RecoverDlc) {
        // ignoring other kinds of background tasks
        return;
      }
      RecoverDlc recoverDlc = RecoverDlc.fromApi(event.field0 as bridge.BackgroundTask_RecoverDlc);
      logger.d("Received a recover dlc event. Status: ${recoverDlc.taskStatus}");

      taskStatus = recoverDlc.taskStatus;

      notifyListeners();
    }
  }
}
