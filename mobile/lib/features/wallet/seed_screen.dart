import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/seed_words.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:get_10101/ffi.dart';

class SeedScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "seed";

  const SeedScreen({super.key});

  @override
  State<SeedScreen> createState() => _SeedScreenState();
}

class _SeedScreenState extends State<SeedScreen> {
  bool checked = false;
  bool visibility = false;

  List<String>? phrase;

  @override
  void initState() {
    phrase = api.getSeedPhrase();
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Backup Seed'),
      ),
      body: SafeArea(
          child: Padding(
        padding: const EdgeInsets.all(20.0),
        child: Column(
          children: [
            Center(
              child: RichText(
                  text: const TextSpan(
                      style: TextStyle(color: Colors.black, fontSize: 18),
                      children: [
                    TextSpan(
                        text: "This is your recovery phrase\n\n",
                        style: TextStyle(fontWeight: FontWeight.bold)),
                    TextSpan(
                        text:
                            "Make sure to write it down as shown here, including both numbers and words.")
                  ])),
            ),
            Expanded(
              child: Container(
                padding: const EdgeInsets.all(20.0),
                child: Column(
                  children: [
                    buildSeedWordsWidget(phrase!, visibility),
                    const SizedBox(height: 10),
                    Center(
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.center,
                        children: [
                          IconButton(
                              icon: visibility
                                  ? const Icon(Icons.visibility)
                                  : const Icon(Icons.visibility_off),
                              onPressed: () {
                                setState(() {
                                  visibility = !visibility;
                                });
                              },
                              tooltip: visibility ? 'Hide Seed' : 'Show Seed'),
                          Text(visibility ? 'Hide Seed' : 'Show Seed')
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
            Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Checkbox(
                          value: checked,
                          onChanged: (bool? changed) {
                            setState(() {
                              checked = changed!;
                            });
                          }),
                      const Text('I have made a backup of my seed'),
                    ],
                  ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      ElevatedButton(
                          onPressed: checked
                              ? () {
                                  Preferences.instance.setUserSeedBackupConfirmed();
                                  context.pop(true);
                                }
                              : null,
                          child: const Text('Done'))
                    ],
                  )
                ],
              ),
            )
          ],
        ),
      )),
    );
  }
}
