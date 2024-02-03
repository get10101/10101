import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/custom_app_bar.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class EmergencyKitScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "emergencykit";

  const EmergencyKitScreen({super.key});

  @override
  State<EmergencyKitScreen> createState() => _EmergencyKitScreenState();
}

class _EmergencyKitScreenState extends State<EmergencyKitScreen> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Container(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const TenTenOneAppBar(title: "Emergency Kit"),
              Container(
                margin: const EdgeInsets.all(10),
                child: Column(
                  children: [
                    const SizedBox(
                      height: 20,
                    ),
                    Container(
                        padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 20),
                        decoration: BoxDecoration(
                            border: Border.all(color: Colors.orange),
                            color: Colors.white,
                            borderRadius: BorderRadius.circular(10)),
                        child: const Row(
                          mainAxisAlignment: MainAxisAlignment.start,
                          children: [
                            Icon(
                              Icons.warning_rounded,
                              color: Colors.orange,
                              size: 22,
                            ),
                            SizedBox(width: 10),
                            Expanded(
                              child: Text(
                                "Only use these emergency kits if you know what you are doing and after consulting with the 10101 team.",
                                softWrap: true,
                                style: TextStyle(fontSize: 16),
                              ),
                            )
                          ],
                        )),
                    const SizedBox(height: 30),
                    OutlinedButton(
                        onPressed: () {
                          showDialog(
                              context: context,
                              builder: (context) {
                                return AlertDialog(
                                    title: const Text("Are you sure?"),
                                    content: const Text(
                                        "Performing that action may break your app state and should only get executed after consulting with the 10101 Team."),
                                    actions: [
                                      TextButton(
                                        onPressed: () => GoRouter.of(context).pop(),
                                        child: const Text('No'),
                                      ),
                                      TextButton(
                                        onPressed: () async {
                                          final messenger = ScaffoldMessenger.of(context);
                                          final orderChangeNotifier =
                                              context.read<OrderChangeNotifier>();
                                          final goRouter = GoRouter.of(context);

                                          try {
                                            await rust.api.setFillingOrdersToFailed();
                                            await orderChangeNotifier.initialize();
                                            goRouter.pop();
                                            showSnackBar(messenger,
                                                "Successfully set filling orders to failed");
                                          } catch (e) {
                                            showSnackBar(messenger,
                                                "Failed to set filling orders to failed. Error: $e");
                                          }
                                        },
                                        child: const Text('Yes'),
                                      ),
                                    ]);
                              });
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
                        child: const Row(mainAxisAlignment: MainAxisAlignment.center, children: [
                          Icon(FontAwesomeIcons.broom),
                          SizedBox(width: 10),
                          Text("Cleanup filling orders", style: TextStyle(fontSize: 16))
                        ]))
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
