import 'package:f_logs/model/flog/flog.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/features/trade/order_submission_status_dialog.dart';
import 'package:provider/provider.dart';

class RolloverChangeNotifier extends ChangeNotifier implements Subscriber {
  late TaskStatus taskStatus;

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_BackgroundNotification) {
      if (event.field0 is! bridge.BackgroundTask_Rollover) {
        // ignoring other kinds of background tasks
        return;
      }

      Rollover rollover = Rollover.fromApi(event.field0 as bridge.BackgroundTask_Rollover);
      FLog.debug(text: "Received a rollover event. Status: ${rollover.taskStatus}");

      taskStatus = rollover.taskStatus;

      if (taskStatus == TaskStatus.pending) {
        // initialize dialog for the pending task
        showDialog(
          context: shellNavigatorKey.currentContext!,
          builder: (context) {
            TaskStatus status = context.watch<RolloverChangeNotifier>().taskStatus;

            // todo(holzeis): Reusing the order submission status dialog is not nice, but it's actually suitable for any task execution that has pending,
            // failed and success states. We may should consider renaming this dialog for its more generic purpose.
            OrderSubmissionStatusDialogType type = OrderSubmissionStatusDialogType.pendingSubmit;
            switch (status) {
              case TaskStatus.pending:
                type = OrderSubmissionStatusDialogType.successfulSubmit;
              case TaskStatus.failed:
                type = OrderSubmissionStatusDialogType.failedFill;
              case TaskStatus.success:
                type = OrderSubmissionStatusDialogType.filled;
            }

            late Widget content = const Text("Rolling over your position");

            return OrderSubmissionStatusDialog(title: "Catching up!", type: type, content: content);
          },
        );
      } else {
        // notify dialog about changed task status
        notifyListeners();
      }
    } else {
      FLog.warning(text: "Received unexpected event: ${event.toString()}");
    }
  }
}
