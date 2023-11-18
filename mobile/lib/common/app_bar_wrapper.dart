import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:go_router/go_router.dart';

class AppBarWrapper extends StatelessWidget {
  const AppBarWrapper({
    Key? key,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    const appBarHeight = 35.0;

    final leadingButton = Row(mainAxisSize: MainAxisSize.min, children: [
      IconButton(
        icon: const Icon(Icons.settings),
        tooltip: 'Settings',
        onPressed: () {
          final String location = GoRouterState.of(context).location;
          GoRouter.of(context).go(SettingsScreen.route, extra: location);
        },
      )
    ]);

    return Container(
        margin: const EdgeInsets.only(left: 10.0, right: 5.0),
        child: AppBar(
            elevation: 0,
            backgroundColor: Colors.transparent,
            iconTheme: const IconThemeData(
                color: tenTenOnePurple,
                // Without adjustment, the icon appears off-center from the title (logo)
                size: appBarHeight - 8.0),
            leading: leadingButton));
  }
}
