import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:get_10101/ffi.dart';

class SeedScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "seed";

  const SeedScreen({super.key});

  @override
  State<SeedScreen> createState() => _SendScreenState();
}

class _SendScreenState extends State<SeedScreen> {
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
    final firstColumn = phrase!
        .getRange(0, 6)
        .toList()
        .asMap()
        .entries
        .map((entry) => SeedWord(entry.value, entry.key + 1, visibility))
        .toList();
    final secondColumn = phrase!
        .getRange(6, 12)
        .toList()
        .asMap()
        .entries
        .map((entry) => SeedWord(entry.value, entry.key + 7, visibility))
        .toList();

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
                    Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [
                        Column(crossAxisAlignment: CrossAxisAlignment.start, children: firstColumn),
                        const SizedBox(width: 10),
                        Column(crossAxisAlignment: CrossAxisAlignment.start, children: secondColumn)
                      ],
                    ),
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
                                  Preferences.instance.setUserSeedBackupConfirmed(true);
                                  context.go(WalletScreen.route);
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

class SeedWord extends StatelessWidget {
  final String? word;
  final int? index;
  final bool visibility;

  const SeedWord(this.word, this.index, this.visibility, {super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
        padding: const EdgeInsets.fromLTRB(0, 10, 0, 0),
        child: Row(
            crossAxisAlignment: visibility ? CrossAxisAlignment.baseline : CrossAxisAlignment.end,
            textBaseline: TextBaseline.alphabetic,
            children: [
              SizedBox(
                width: 25.0,
                child: Text(
                  '#$index',
                  style: TextStyle(fontSize: 12, color: Colors.grey[600]),
                ),
              ),
              const SizedBox(width: 5),
              visibility
                  ? Text(
                      word!,
                      style: const TextStyle(fontSize: 20, fontWeight: FontWeight.bold),
                    )
                  : Container(
                      color: Colors.grey[300], child: const SizedBox(width: 100, height: 24))
            ]));
  }
}
