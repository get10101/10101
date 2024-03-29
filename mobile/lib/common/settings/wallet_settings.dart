import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/ffi.dart' as rust;

class WalletSettings extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "walletSettings";

  const WalletSettings({super.key});

  @override
  State<WalletSettings> createState() => _WalletSettingsState();
}

class _WalletSettingsState extends State<WalletSettings> {
  var lookAheadController = TextEditingController();
  var syncing = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
          child: Padding(
        padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
        child: Column(
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Expanded(
                  child: Stack(
                    children: [
                      GestureDetector(
                        child: Container(
                            alignment: AlignmentDirectional.topStart,
                            decoration: BoxDecoration(
                                color: Colors.transparent, borderRadius: BorderRadius.circular(10)),
                            width: 70,
                            child: const Icon(
                              Icons.arrow_back_ios_new_rounded,
                              size: 22,
                            )),
                        onTap: () {
                          GoRouter.of(context).pop();
                        },
                      ),
                      const Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Text(
                            "Wallet Settings",
                            style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 20,
            ),
            Expanded(
                child: SingleChildScrollView(
              child: Padding(
                padding: const EdgeInsets.all(8.0),
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.start,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      "Full sync \n",
                      style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                    ),
                    const Text(
                      "Select the stop gap and confirm to perform a full wallet sync.\n\nThe stop gap determines how many consecutive unused addresses the wallet will have to find to stop syncing. The bigger the gap, the longer the sync will take. Hence, we recommend syncing incrementally.\n\nFor example: use a stop gap of 5, wait for the sync to complete and check your balance; if your balance still seems incorrect, come back here and use a stop gap of 10, etc.",
                      style: TextStyle(fontSize: 18),
                    ),
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 16),
                      child: Stack(
                        alignment: Alignment.centerRight,
                        children: [
                          TextFormField(
                            inputFormatters: <TextInputFormatter>[
                              FilteringTextInputFormatter.digitsOnly
                            ],
                            keyboardType: TextInputType.number,
                            controller: lookAheadController,
                            decoration: const InputDecoration(
                              border: UnderlineInputBorder(),
                              labelText: 'Stop Gap',
                            ),
                          ),
                          Visibility(
                              visible: !syncing,
                              replacement: const CircularProgressIndicator(),
                              child: IconButton(
                                icon: const Icon(
                                  Icons.check,
                                  color: Colors.green,
                                ),
                                onPressed: () async {
                                  final messenger = ScaffoldMessenger.of(context);
                                  try {
                                    var gap = lookAheadController.value.text;
                                    var gapAsNumber = int.parse(gap);

                                    setState(() {
                                      syncing = true;
                                    });

                                    await rust.api.fullSync(stopGap: gapAsNumber);
                                    showSnackBar(messenger, "Successfully synced for new gap.");

                                    setState(() {
                                      syncing = false;
                                    });
                                  } catch (exception) {
                                    logger.e("Failed to complete full sync $exception");
                                    showSnackBar(
                                        messenger, "Error when running full sync $exception");
                                  } finally {
                                    setState(() {
                                      syncing = false;
                                    });
                                  }
                                },
                              ))
                        ],
                      ),
                    )
                  ],
                ),
              ),
            )),
          ],
        ),
      )),
    );
  }
}
