import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:provider/provider.dart';

class CollabRevertChangeNotifier extends ChangeNotifier implements Subscriber {
  late TaskStatus taskStatus;

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

      if (taskStatus == TaskStatus.pending) {
        // initialize dialog for the pending task
        showDialog(
          context: shellNavigatorKey.currentContext!,
          builder: (context) {
            TaskStatus status = context.watch<CollabRevertChangeNotifier>().taskStatus;
            late Widget content = const Text("Your channel has been closed collaboratively!");
            return TaskStatusDialog(
                title: "Collaborative Channel Close!", status: status, content: content);
          },
        );
      } else {
        // notify dialog about changed task status
        notifyListeners();
      }
    }
  }
}
