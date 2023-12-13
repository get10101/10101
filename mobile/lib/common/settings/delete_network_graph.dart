import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/custom_app_bar.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/ffi.dart' as rust;

class DeleteNetworkGraphScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "deletenetworkgraph";

  const DeleteNetworkGraphScreen({super.key});

  @override
  State<DeleteNetworkGraphScreen> createState() => _DeleteNetworkGraphScreenState();
}

class _DeleteNetworkGraphScreenState extends State<DeleteNetworkGraphScreen> {
  bool deletedNetworkGraph = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Container(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const TenTenOneAppBar(title: "Delete Network Graph"),
              Container(
                margin: const EdgeInsets.all(10),
                child: Column(
                  children: [
                    const SizedBox(
                      height: 20,
                    ),
                    const Text(
                      "If you are having troubles sending a payment, it may help to delete and recreate you network graph.",
                      style: TextStyle(fontSize: 18, fontWeight: FontWeight.w400),
                    ),
                    const SizedBox(height: 30),
                    Visibility(
                        visible: deletedNetworkGraph,
                        replacement: OutlinedButton(
                            onPressed: () {
                              final messenger = ScaffoldMessenger.of(context);
                              try {
                                rust.api.deleteNetworkGraph();
                                setState(() => deletedNetworkGraph = true);
                                showSnackBar(messenger, "Successfully deleted network graph");
                              } catch (e) {
                                showSnackBar(
                                    messenger, "Failed to delete network graph. Error: $e");
                              }
                            },
                            style: ButtonStyle(
                              fixedSize: MaterialStateProperty.all(const Size(double.infinity, 50)),
                              iconSize: MaterialStateProperty.all<double>(20.0),
                              elevation: MaterialStateProperty.all<double>(0),
                              // this reduces the shade
                              side: MaterialStateProperty.all(
                                  const BorderSide(width: 1.0, color: tenTenOnePurple)),
                              padding: MaterialStateProperty.all<EdgeInsetsGeometry>(
                                const EdgeInsets.fromLTRB(20, 12, 20, 12),
                              ),
                              shape: MaterialStateProperty.all<RoundedRectangleBorder>(
                                RoundedRectangleBorder(
                                  borderRadius: BorderRadius.circular(8.0),
                                ),
                              ),
                              backgroundColor: MaterialStateProperty.all<Color>(Colors.transparent),
                            ),
                            child: const Row(
                                mainAxisAlignment: MainAxisAlignment.center,
                                children: [
                                  Icon(Icons.delete),
                                  SizedBox(width: 10),
                                  Text("Delete Network Graph")
                                ])),
                        child: Container(
                            padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 20),
                            decoration: BoxDecoration(
                                border: Border.all(color: tenTenOnePurple),
                                color: Colors.white,
                                borderRadius: BorderRadius.circular(10)),
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.start,
                              children: [
                                Icon(
                                  Icons.info_outline,
                                  color: tenTenOnePurple.shade800,
                                  size: 22,
                                ),
                                const SizedBox(width: 10),
                                const Text(
                                  "Restart the app to recreate your\nnetwork graph.",
                                  softWrap: true,
                                  overflow: TextOverflow.ellipsis,
                                  maxLines: 4,
                                ),
                              ],
                            ))),
                  ],
                ),
              )
            ],
          ),
        ),
      ),
    );
  }
}
