import 'package:flutter/material.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/settings_screen.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/features/wallet/status_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class AppBarWrapper extends StatelessWidget {
  const AppBarWrapper({
    Key? key,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final currentRoute = GoRouterState.of(context).location;
    const appBarHeight = 35.0;

    ChannelStatusNotifier channelStatusNotifier = context.watch<ChannelStatusNotifier>();

    var actionButtons = [
      IconButton(
        icon: channelStatusNotifier.isClosing()
            ? const Icon(Icons.thermostat, color: Colors.red)
            : const Icon(Icons.thermostat),
        tooltip: 'Status',
        onPressed: () {
          context.go(WalletStatusScreen.route);
        },
      )
    ];

    Widget? leadingButton;

    if (currentRoute == WalletScreen.route) {
      leadingButton = Row(mainAxisSize: MainAxisSize.min, children: [
        IconButton(
          icon: const Icon(Icons.settings),
          tooltip: 'Settings',
          onPressed: () {
            Navigator.of(context).push(_createRoute());
          },
        )
      ]);
    }

    if (currentRoute == TradeScreen.route) {
      leadingButton = Row(mainAxisSize: MainAxisSize.min, children: [
        IconButton(
          icon: const Icon(Icons.settings),
          tooltip: 'Settings',
          onPressed: () {
            Navigator.of(context).push(_createRoute());
          },
        )
      ]);
    }

    return Container(
        margin: const EdgeInsets.only(left: 6.0),
        child: AppBar(
          elevation: 0,
          backgroundColor: Colors.transparent,
          iconTheme: const IconThemeData(
              color: tenTenOnePurple,
              // Without adjustment, the icon appears off-center from the title (logo)
              size: appBarHeight - 8.0),
          leading: leadingButton,
          title: SizedBox(
            height: appBarHeight - 10.0,
            child: Image.asset('assets/10101_logo_icon.png'),
          ),
          actions: actionButtons,
        ));
  }
}

Route _createRoute() {
  return PageRouteBuilder(
    pageBuilder: (context, animation, secondaryAnimation) => const SettingsScreen(),
    transitionsBuilder: (context, animation, secondaryAnimation, child) {
      const begin = Offset(-1.0, 0.0);
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
