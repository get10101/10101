import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/services/version_service.dart';
import 'package:get_10101/services/settings_service.dart';
import 'package:provider/provider.dart';

class AppInfoScreen extends StatefulWidget {
  const AppInfoScreen({super.key});

  @override
  State<AppInfoScreen> createState() => _AppInfoScreenState();
}

class _AppInfoScreenState extends State<AppInfoScreen> {
  EdgeInsets margin = const EdgeInsets.all(10);

  String _version = "";
  String _nodeId = "";
  String _commit = "not available";
  String _branch = "not available";

  @override
  void initState() {
    Future.wait<dynamic>([
      context.read<VersionService>().fetchVersion(),
      context.read<SettingsService>().getNodeId()
    ]).then((value) {
      final version = value[0];
      final nodeId = value[1];

      setState(() {
        _commit = version.commitHash;
        _branch = version.branch;
        _version = version.version;
        _nodeId = nodeId;
      });
    });

    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Container(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
          child: Column(
            children: [
              Column(
                children: [
                  Container(
                    margin: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 10),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          "NODE INFO",
                          style: TextStyle(color: Colors.grey, fontSize: 17),
                        ),
                        const SizedBox(
                          height: 10,
                        ),
                        Container(
                            decoration: BoxDecoration(
                                color: Colors.white, borderRadius: BorderRadius.circular(15)),
                            child: moreInfo(context,
                                title: "Node Id", info: _nodeId, showCopyButton: true))
                      ],
                    ),
                  ),
                  Container(
                    margin: const EdgeInsets.all(10),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          "BUILD INFO",
                          style: TextStyle(color: Colors.grey, fontSize: 18),
                        ),
                        const SizedBox(
                          height: 10,
                        ),
                        Container(
                            decoration: BoxDecoration(
                                color: Colors.white, borderRadius: BorderRadius.circular(15)),
                            child: Column(
                              children: [
                                moreInfo(context, title: "Version", info: _version),
                                moreInfo(context,
                                    title: "Commit Hash", info: _commit, showCopyButton: true),
                                moreInfo(context,
                                    title: "Branch", info: _branch, showCopyButton: kDebugMode)
                              ],
                            ))
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 10)
            ],
          )),
    );
  }
}

Widget moreInfo(BuildContext context,
    {required String title, required String info, bool showCopyButton = false}) {
  return Container(
    padding: const EdgeInsets.all(15),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              title,
              style:
                  const TextStyle(fontSize: 17, fontWeight: FontWeight.w400, color: Colors.black),
            ),
            const SizedBox(height: 7),
            showCopyButton
                ? SizedBox(
                    width: 400,
                    child: Text(
                      info,
                      softWrap: true,
                      maxLines: 4,
                      style: TextStyle(
                          fontSize: 18, fontWeight: FontWeight.w300, color: Colors.grey.shade700),
                    ))
                : const SizedBox()
          ],
        ),
        showCopyButton
            ? GestureDetector(
                onTap: () async {
                  showSnackBar(ScaffoldMessenger.of(context), "Copied $info");
                  await Clipboard.setData(ClipboardData(text: info));
                },
                child: Icon(
                  Icons.copy,
                  size: 17,
                  color: tenTenOnePurple.shade800,
                ),
              )
            : Text(
                info,
                style: TextStyle(
                    fontSize: 18, fontWeight: FontWeight.w300, color: Colors.grey.shade700),
              )
      ],
    ),
  );
}
