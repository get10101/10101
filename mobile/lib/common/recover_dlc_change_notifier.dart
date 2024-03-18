import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:provider/provider.dart';

class RecoverDlcChangeNotifier extends ChangeNotifier implements Subscriber {
  late TaskStatus taskStatus;

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

      if (taskStatus == TaskStatus.pending) {
        while (shellNavigatorKey.currentContext == null) {
          await Future.delayed(const Duration(milliseconds: 100)); // Adjust delay as needed
        }
        // initialize dialog for the pending task
        showDialog(
          context: shellNavigatorKey.currentContext!,
          builder: (context) {
            TaskStatus status = context.watch<RecoverDlcChangeNotifier>().taskStatus;
            late Widget content = const Text("Recovering your dlc channel");
            return TaskStatusDialog(title: "Catching up!", status: status, content: content);
          },
        );
      } else {
        // notify dialog about changed task status
        notifyListeners();
      }
    }
  }
}
