import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
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
