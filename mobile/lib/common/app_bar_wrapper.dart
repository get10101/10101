import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/features/trade/settings_screen.dart';
import 'package:go_router/go_router.dart';

class AppBarWrapper extends StatelessWidget {
  const AppBarWrapper({
    Key? key,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    const appBarHeight = 35.0;

    var actionButtons = <Widget>[];
    Widget leadingButton = IconButton(
      icon: const Icon(Icons.settings),
      tooltip: 'Settings',
      onPressed: () {
        context.go(TradeSettingsScreen.route);
      },
    );

    return Container(
        margin: const EdgeInsets.only(left: 2.0),
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
