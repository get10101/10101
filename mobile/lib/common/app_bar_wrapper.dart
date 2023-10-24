import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/status_icon_button.dart';

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
          Navigator.of(context).push(_createSettingsRoute());
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
            leading: leadingButton,
            actions: const [
              StatusIconButton(),
            ]));
  }
}

Route _createSettingsRoute() {
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
