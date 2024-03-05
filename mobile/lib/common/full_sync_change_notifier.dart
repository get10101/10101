import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:provider/provider.dart';

class FullSyncChangeNotifier extends ChangeNotifier implements Subscriber {
  late TaskStatus taskStatus;

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

      if (taskStatus == TaskStatus.pending) {
        while (shellNavigatorKey.currentContext == null) {
          await Future.delayed(const Duration(milliseconds: 100)); // Adjust delay as needed
        }

        // initialize dialog for the pending task
        showDialog(
          context: shellNavigatorKey.currentContext!,
          builder: (context) {
            TaskStatus status = context.watch<FullSyncChangeNotifier>().taskStatus;

            Widget content;
            switch (status) {
              case TaskStatus.pending:
                content = const Text("Waiting for on-chain sync to complete");
              case TaskStatus.success:
                content = const Text(
                    "Full on-chain sync completed. If your balance is still incomplete, go to Wallet Settings to trigger further syncs.");
              case TaskStatus.failed:
                content = const Text(
                    "Full on-chain sync failed. You can keep trying by shutting down the app and restarting.");
            }

            return TaskStatusDialog(title: "Full wallet sync", status: status, content: content);
          },
        );
      } else {
        // notify dialog about changed task status
        notifyListeners();
      }
    }
  }
}
