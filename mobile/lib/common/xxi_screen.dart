import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/background_task_change_notifier.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/features/trade/async_order_change_notifier.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/features/trade/trade_dialog.dart';
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
    final asyncTrade = context.watch<AsyncOrderChangeNotifier>().asyncTrade;

    final task = events.isEmpty ? null : events.peek;

    WidgetsBinding.instance.addPostFrameCallback((_) {
      final taskStatusDialog = getTaskStatusDialog(task);

      if (taskStatusDialog != null && activeTask == null) {
        // only create a new dialog if there isn't an active task already.
        showDialog(
            context: context,
            builder: (context) {
              // watch task updates from within the dialog.
              final task = context.watch<BackgroundTaskChangeNotifier>().events.pop();
              if (activeTask != null && task.type != activeTask!.type) {
                logger.w("Ignoring task event $task while $activeTask is still active!");
              } else {
                // update the active task to the last event received on the stack.
                activeTask = task;
              }
              return getTaskStatusDialog(task)!;
            });
      } else if (asyncTrade != null) {
        if (asyncTrade.orderReason == OrderReason.manual) {
          showDialog(
              context: context,
              useRootNavigator: true,
              barrierDismissible: false,
              builder: (BuildContext context) {
                return const TradeDialog();
              });
        } else {
          showDialog(
            context: context,
            builder: (context) {
              Order? asyncOrder = context.watch<AsyncOrderChangeNotifier>().asyncOrder;

              TaskStatus status = TaskStatus.pending;
              switch (asyncOrder?.state) {
                case OrderState.open:
                case OrderState.filling:
                  status = TaskStatus.pending;
                case OrderState.failed:
                case OrderState.rejected:
                  status = TaskStatus.failed;
                case OrderState.filled:
                  status = TaskStatus.success;
                case null:
                  status = TaskStatus.pending;
              }

              late Widget content;
              switch (asyncTrade.orderReason) {
                case OrderReason.expired:
                  content = const Text("Your position has been closed due to expiry.");
                case OrderReason.liquidated:
                  content = const Text("Your position has been closed due to liquidation.");
                case OrderReason.manual:
                  logger.e("A manual order should not appear as an async trade!");
                  content = Container();
              }

              return TaskStatusDialog(title: "Catching up!", status: status, content: content);
            },
          );
        }

        // remove the async trade from the change notifier state, marking that the dialog has been created.
        context.read<AsyncOrderChangeNotifier>().removeAsyncTrade();
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
      TaskType.asyncTrade || TaskType.unknown || null => null
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
