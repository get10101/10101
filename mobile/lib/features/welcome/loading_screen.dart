import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/backend.dart';
import 'package:get_10101/features/welcome/error_screen.dart';
import 'package:get_10101/features/welcome/onboarding.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/features/welcome/welcome_screen.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:get_10101/util/file.dart';
import 'package:go_router/go_router.dart';

class LoadingScreen extends StatefulWidget {
  static const route = "/loading";

  final LoadingScreenTask? task;

  const LoadingScreen({super.key, this.task});

  @override
  State<LoadingScreen> createState() => _LoadingScreenState();
}

class _LoadingScreenState extends State<LoadingScreen> {
  String message = "Welcome to 10101";

  @override
  void initState() {
    initAsync();
    super.initState();
  }

  Future<void> initAsync() async {
    var skipBetaRegistration = false;
    try {
      await widget.task?.future;
    } catch (err, stackTrace) {
      final task = widget.task!;
      final taskErr = task.error(err);
      skipBetaRegistration = task.skipBetaRegistrationOnFail;
      logger.e(taskErr, error: err, stackTrace: stackTrace);
      showSnackBar(ScaffoldMessenger.of(rootNavigatorKey.currentContext!), taskErr);
    }

    final [position, seedPresent, backupRequired, registeredForBeta] = await Future.wait<dynamic>([
      Preferences.instance.getOpenPosition(),
      isSeedFilePresent(),
      Preferences.instance.isFullBackupRequired(),
      Preferences.instance.isRegisteredForBeta(),
    ]);
    FlutterNativeSplash.remove();

    if (seedPresent) {
      if (!registeredForBeta && !skipBetaRegistration) {
        logger.w("Registering for beta program despite having a seed; "
            "onboarding flow was probably previously interrupted");
        setState(() => message = "Registering for beta program");

        try {
          await resumeRegisterForBeta();
        } catch (err, stackTrace) {
          const failed = "Failed to register for beta program";
          showSnackBar(ScaffoldMessenger.of(rootNavigatorKey.currentContext!), "$failed.");
          logger.e(failed, error: err, stackTrace: stackTrace);
        }
      }

      if (backupRequired) {
        setState(() => message = "Creating initial backup!");
        fullBackup().then((value) {
          Preferences.instance.setFullBackupRequired(false).then((value) {
            start(rootNavigatorKey.currentContext!, position);
          });
        }).catchError((error) {
          logger.e("Failed to run full backup. $error");
          showSnackBar(
              ScaffoldMessenger.of(rootNavigatorKey.currentContext!), "Failed to start 10101!");
        });
      } else {
        start(rootNavigatorKey.currentContext!, position);
      }
    } else {
      // No seed file: let the user choose whether they want to create a new
      // wallet or import their old one
      Preferences.instance.setFullBackupRequired(false).then((value) {
        GoRouter.of(context).go(Onboarding.route);
      });
    }
  }

  void start(BuildContext context, String? position) {
    setState(() => message = "Starting 10101");
    runBackend(context).then((value) {
      logger.i("Backend started");

      switch (position) {
        case TradeScreen.label:
          GoRouter.of(context).go(TradeScreen.route);
        default:
          GoRouter.of(context).go(WalletScreen.route);
      }
    }).catchError((error) {
      logger.e("Failed to start backend. $error");
      GoRouter.of(context).go(ErrorScreen.route);
      showSnackBar(ScaffoldMessenger.of(context), "Failed to start 10101! $error");
    });
  }

  @override
  Widget build(BuildContext context) {
    return AnnotatedRegion<SystemUiOverlayStyle>(
        value: SystemUiOverlayStyle.dark,
        child: Scaffold(
            backgroundColor: Colors.white,
            body: ScrollableSafeArea(
                child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Center(
                  child: Image.asset('assets/10101_logo_icon.png', width: 150, height: 150),
                ),
                const SizedBox(height: 40),
                const Center(child: CircularProgressIndicator()),
                const SizedBox(height: 15),
                Text(message)
              ],
            ))));
  }
}

/// Some operation carried out whilst the loading screen is displayed
class LoadingScreenTask {
  /// The future of the task itself
  final Future<void> future;

  /// Create the snackbar text error to display if the task fails
  final String Function(dynamic) error;

  /// Whether to skip the beta registration if this task fails. This should
  /// be `true` if the task's `future` did itself try to register the user for
  /// beta.
  final bool skipBetaRegistrationOnFail;

  LoadingScreenTask(
      {required this.future, required this.error, this.skipBetaRegistrationOnFail = false});
}
