import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/custom_app_bar.dart';
import 'package:get_10101/common/dlc_channel_change_notifier.dart';
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
  String? _dlcChannelId;

  @override
  Widget build(BuildContext context) {
    final bridge.Config config = context.read<bridge.Config>();
    final dlcChannelChangeNotifier = context.read<DlcChannelChangeNotifier>();

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
                  child: Column(children: [
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
                    EmergencyKitButton(
                        icon: const Icon(FontAwesomeIcons.broom),
                        title: "Cleanup filling orders",
                        onPressed: () async {
                          final messenger = ScaffoldMessenger.of(context);
                          final orderChangeNotifier = context.read<OrderChangeNotifier>();
                          final goRouter = GoRouter.of(context);

                          try {
                            await rust.api.setFillingOrdersToFailed();
                            await orderChangeNotifier.initialize();
                            showSnackBar(messenger, "Successfully set filling orders to failed");
                          } catch (e) {
                            showSnackBar(
                                messenger, "Failed to set filling orders to failed. Error: $e");
                          }

                          goRouter.pop();
                        }),
                    const SizedBox(height: 30),
                    Row(
                      children: [
                        Expanded(
                            child: TextField(
                          onChanged: (value) => _dlcChannelId = value,
                          decoration: InputDecoration(
                            border: const OutlineInputBorder(),
                            hintText: "Copy from Settings > Channel",
                            labelText: "Dlc Channel Id",
                            labelStyle: const TextStyle(color: Colors.black87),
                            filled: true,
                            fillColor: Colors.white,
                            errorStyle: TextStyle(
                              color: Colors.red[900],
                            ),
                          ),
                        )),
                        IconButton(
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
                                              final goRouter = GoRouter.of(context);

                                              try {
                                                await dlcChannelChangeNotifier
                                                    .deleteDlcChannel(_dlcChannelId ?? "");
                                                showSnackBar(messenger,
                                                    "Successfully deleted dlc channel with id $_dlcChannelId");
                                              } catch (error) {
                                                showSnackBar(messenger, "$error");
                                              }
                                              goRouter.pop();
                                            },
                                            child: const Text('Yes'),
                                          ),
                                        ]);
                                  });
                            },
                            icon: const Icon(
                              Icons.delete,
                              size: 32,
                            ))
                      ],
                    ),
                    const SizedBox(
                      height: 30,
                    ),
                    Visibility(
                        visible: config.network == "regtest",
                        child: EmergencyKitButton(
                            icon: const Icon(FontAwesomeIcons.broom),
                            title: "Reset answered poll cache",
                            onPressed: () {
                              final messenger = ScaffoldMessenger.of(context);
                              try {
                                rust.api.resetAllAnsweredPolls();
                                showSnackBar(messenger,
                                    "Successfully reset answered polls - You can now answer them again");
                              } catch (e) {
                                showSnackBar(messenger, "Failed to reset answered polls: $e");
                              }
                            })),
                  ])),
            ],
          ),
        ),
      ),
    );
  }
}

class EmergencyKitButton extends StatelessWidget {
  final Icon icon;
  final String title;
  final VoidCallback onPressed;

  const EmergencyKitButton(
      {super.key, required this.icon, required this.title, required this.onPressed});

  @override
  Widget build(BuildContext context) {
    return OutlinedButton(
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
                        onPressed: onPressed,
                        child: const Text('Yes'),
                      ),
                    ]);
              });
        },
        style: ButtonStyle(
          fixedSize: MaterialStateProperty.all(const Size(double.infinity, 50)),
          iconSize: MaterialStateProperty.all<double>(20.0),
          elevation: MaterialStateProperty.all<double>(0),
          side: MaterialStateProperty.all(const BorderSide(width: 1.0, color: tenTenOnePurple)),
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
        child: Row(mainAxisAlignment: MainAxisAlignment.center, children: [
          icon,
          const SizedBox(width: 10),
          Text(title, style: const TextStyle(fontSize: 16))
        ]));
  }
}
