import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/background_task_change_notifier.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/features/trade/error_details.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'dart:convert';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/util/coordinator_version.dart';
import 'package:http/http.dart' as http;
import 'package:provider/provider.dart';
import 'package:version/version.dart';

class XXIScreen extends StatefulWidget {
  final Widget child;

  const XXIScreen({super.key, required this.child});

  @override
  State<XXIScreen> createState() => _XXIScreenState();
}

class _XXIScreenState extends State<XXIScreen> {
  BackgroundTask? activeTask;

  @override
  void initState() {
    final config = context.read<bridge.Config>();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      compareCoordinatorVersion(config);
    });

    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final events = context.watch<BackgroundTaskChangeNotifier>().events;

    final task = events.isEmpty ? null : events.peek;

    WidgetsBinding.instance.addPostFrameCallback((_) {
      final taskStatusDialog = getTaskStatusDialog(task);

      if (taskStatusDialog != null && activeTask == null) {
        // only create a new dialog if there isn't an active task already.
        showGeneralDialog(
            context: context,
            useRootNavigator: true,
            barrierDismissible: false,
            transitionBuilder: (context, animation, __, child) {
              final curvedValue = Curves.easeInOutBack.transform(animation.value) - 1.5;
              return Transform(
                transform: Matrix4.translationValues(0.0, curvedValue * 100, 0.0),
                child: Opacity(
                  opacity: animation.value,
                  child: child,
                ),
              );
            },
            pageBuilder: (context, _, __) {
              // watch task updates from within the dialog.
              final task = context.watch<BackgroundTaskChangeNotifier>().events.pop();
              if (activeTask != null && task.type != activeTask!.type) {
                if (activeTask!.type == TaskType.recover && task.type == TaskType.asyncTrade) {
                  // in case a trade protocol finishes we get an async trade task status update, but
                  // there might have been a restart in between as why the recover task dialog is
                  // shown. Since these two tasks are different we have to handle this case here
                  // and only update the task status of the active task.
                  activeTask!.status = task.status;
                } else {
                  logger.w("Ignoring task event $task while $activeTask is still active!");
                }
              } else {
                // update the active task to the last event received on the stack.
                activeTask = task;
              }
              return getTaskStatusDialog(activeTask)!;
            });
      }
    });

    return AnnotatedRegion<SystemUiOverlayStyle>(
        value: SystemUiOverlayStyle.dark, child: Scaffold(body: widget.child));
  }

  TaskStatusDialog? getTaskStatusDialog(BackgroundTask? task) {
    return switch (task?.type) {
      TaskType.rollover => TaskStatusDialog(
          title: "Rollover",
          status: task!.status,
          content: const Text("Rolling over your position"),
          onClose: () => activeTask = null),
      TaskType.collaborativeRevert => TaskStatusDialog(
          title: "Collaborative Channel Close!",
          status: task!.status,
          content: const Text("Your channel has been closed collaboratively!"),
          onClose: () => activeTask = null),
      TaskType.fullSync => TaskStatusDialog(
          title: "Full wallet sync",
          status: task!.status,
          content: switch (task.status) {
            TaskStatus.pending => const Text("Waiting for on-chain sync to complete"),
            TaskStatus.success => const Text(
                "Full on-chain sync completed. If your balance is still incomplete, go to Wallet Settings to trigger further syncs."),
            TaskStatus.failed => const Text(
                "Full on-chain sync failed. You can keep trying by shutting down the app and restarting.")
          },
          onClose: () => activeTask = null),
      TaskType.recover => TaskStatusDialog(
          title: "Catching up!",
          status: task!.status,
          content: const Text("Recovering your dlc channel"),
          onClose: () => activeTask = null),
      TaskType.asyncTrade => TaskStatusDialog(
          title: "Executing Order!",
          status: task!.status,
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              switch (task.status) {
                TaskStatus.pending =>
                  const Text("Please do not close the app while the trade is executed."),
                TaskStatus.success => const Text("The order has been successfully executed!"),
                TaskStatus.failed => const Text("Something went wrong!")
              },
              if (task.status == TaskStatus.failed && task.error != null)
                ErrorDetails(details: task.error!)
            ],
          ),
          onClose: () => activeTask = null),
      TaskType.unknown || null => null
    };
  }

  /// Compare the version of the coordinator with the version of the app
  ///
  /// - If the coordinator is newer, suggest to update the app.
  /// - If the app is newer, log it.
  /// - If the coordinator cannot be reached, show a warning that the app may not function properly.
  void compareCoordinatorVersion(bridge.Config config) {
    Future.wait<dynamic>([
      PackageInfo.fromPlatform(),
      http.get(Uri.parse('http://${config.host}:${config.httpPort}/api/version'))
    ]).then((value) {
      final packageInfo = value[0];
      final response = value[1];

      final clientVersion = Version.parse(packageInfo.version);
      final coordinatorVersion = CoordinatorVersion.fromJson(jsonDecode(response.body));
      logger.i("Coordinator version: ${coordinatorVersion.version.toString()}");

      if (coordinatorVersion.version > clientVersion) {
        logger.w("Client out of date. Current version: ${clientVersion.toString()}");
        showDialog(
            context: context,
            builder: (context) => AlertDialog(
                    title: const Text("Update available"),
                    content: Text("A new version of 10101 is available: "
                        "${coordinatorVersion.version.toString()}.\n\n"
                        "Please note that if you do not update 10101, the app"
                        " may not function properly."),
                    actions: [
                      TextButton(
                        onPressed: () => Navigator.pop(context, 'OK'),
                        child: const Text('OK'),
                      ),
                    ]));
      } else if (coordinatorVersion.version < clientVersion) {
        logger.w("10101 is newer than coordinator: ${coordinatorVersion.version.toString()}");
      } else {
        logger.i("Client is up to date: ${clientVersion.toString()}");
      }
    }).catchError((e) {
      logger.e("Error getting coordinator version: ${e.toString()}");

      showDialog(
          context: context,
          builder: (context) => AlertDialog(
                  title: const Text("Cannot reach coordinator"),
                  content: const Text("Please check your Internet connection.\n"
                      "Please note that without Internet access, the app "
                      "functionality is severely limited."),
                  actions: [
                    TextButton(
                      onPressed: () => Navigator.pop(context, 'OK'),
                      child: const Text('OK'),
                    ),
                  ]));
    });
  }
}
