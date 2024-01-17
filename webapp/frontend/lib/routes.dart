import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/main.dart';
import 'package:go_router/go_router.dart';

GoRouter createRoutes() {
  return GoRouter(
      navigatorKey: rootNavigatorKey,
      initialLocation: MyHomePage.route,
      routes: <RouteBase>[
        GoRoute(
          path: MyHomePage.route,
          pageBuilder: (context, state) => const NoTransitionPage<void>(
            child: MyHomePage(
              title: '10101 Web app',
            ),
          ),
        ),
      ]);
}
