import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/collab_revert_change_notifier.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/full_sync_change_notifier.dart';
import 'package:get_10101/common/recover_dlc_change_notifier.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/features/trade/async_order_change_notifier.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/features/trade/rollover_change_notifier.dart';
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
    final recoverTaskStatus = context.watch<RecoverDlcChangeNotifier>().taskStatus;
    final rolloverTaskStatus = context.watch<RolloverChangeNotifier>().taskStatus;
    final asyncTrade = context.watch<AsyncOrderChangeNotifier>().asyncTrade;
    final fullSyncTaskStatus = context.watch<FullSyncChangeNotifier>().taskStatus;
    final collabRevertTaskStatus = context.watch<CollabRevertChangeNotifier>().taskStatus;

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (recoverTaskStatus == TaskStatus.pending) {
        showDialog(
          context: context,
          builder: (context) {
            TaskStatus status = context.watch<RecoverDlcChangeNotifier>().taskStatus;
            late Widget content = const Text("Recovering your dlc channel");
            return TaskStatusDialog(title: "Catching up!", status: status, content: content);
          },
        );
      }
      if (rolloverTaskStatus == TaskStatus.pending) {
        showDialog(
          context: context,
          builder: (context) {
            TaskStatus status = context.watch<RolloverChangeNotifier>().taskStatus;
            late Widget content = const Text("Rolling over your position");
            return TaskStatusDialog(title: "Catching up!", status: status, content: content);
          },
        );
      }
      if (asyncTrade != null) {
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

        // remove the async trade from the change notifier state, marking that the dialog has been created.
        context.read<AsyncOrderChangeNotifier>().removeAsyncTrade();
      }
      if (fullSyncTaskStatus == TaskStatus.pending) {
        showDialog(
          context: context,
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
      }
      if (collabRevertTaskStatus == TaskStatus.pending) {
        showDialog(
          context: context,
          builder: (context) {
            TaskStatus status = context.watch<CollabRevertChangeNotifier>().taskStatus;
            late Widget content = const Text("Your channel has been closed collaboratively!");
            return TaskStatusDialog(
                title: "Collaborative Channel Close!", status: status, content: content);
          },
        );
      }
    });

    return AnnotatedRegion<SystemUiOverlayStyle>(
        value: SystemUiOverlayStyle.dark, child: Scaffold(body: widget.child));
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
