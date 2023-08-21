import 'package:flutter/material.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/service_status_notifier.dart';
import 'package:get_10101/common/status_screen.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as rust;
import 'package:provider/provider.dart';

class StatusIconButton extends StatelessWidget {
  const StatusIconButton({super.key});

  @override
  Widget build(BuildContext context) {
    final channelStatusNotifier = context.watch<ChannelStatusNotifier>();
    final serviceStatusNotifier = context.watch<ServiceStatusNotifier>();

    final overallStatus = serviceStatusNotifier.overall();

    return IconButton(
      icon: channelStatusNotifier.isClosing() || overallStatus == rust.ServiceStatus.Offline
          ? const Icon(Icons.thermostat, color: Colors.red)
          : overallStatus == rust.ServiceStatus.Unknown
              ? const Icon(Icons.thermostat, color: Colors.yellow)
              : const Icon(Icons.thermostat),
      tooltip: 'Status',
      onPressed: () {
        Navigator.of(context).push(_createStatusRoute());
      },
    );
  }
}

Route _createStatusRoute() {
  return PageRouteBuilder(
    pageBuilder: (context, animation, secondaryAnimation) => const StatusScreen(),
    transitionsBuilder: (context, animation, secondaryAnimation, child) {
      const begin = Offset(1.0, 0.0);
      const end = Offset.zero;
      const curve = Curves.ease;

      var tween = Tween(begin: begin, end: end).chain(CurveTween(curve: curve));

      return SlideTransition(
        position: animation.drive(tween),
        child: child,
      );
    },
  );
}
