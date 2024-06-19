import 'package:flutter/material.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/settings/seed_words.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/ffi.dart';

class SeedScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
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
      body: ScrollableSafeArea(
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
                            "Backup",
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
            Container(
              margin: const EdgeInsets.all(10),
              child: Center(
                child: RichText(
                    text: const TextSpan(
                        style: TextStyle(color: Colors.black, fontSize: 18),
                        children: [
                      TextSpan(
                          text:
                              "The recovery phrase (sometimes called a seed), is a list of 12 English words. It allows you to recover full access to your funds if needed\n\n"),
                      TextSpan(
                          text: "Do not share this seed with anyone. ",
                          style: TextStyle(fontWeight: FontWeight.bold)),
                      TextSpan(
                          text:
                              "Beware of phising. The developers of 10101 will never ask for your seed.\n\n"),
                      TextSpan(
                          text: "Do not lose this seed. ",
                          style: TextStyle(fontWeight: FontWeight.bold)),
                      TextSpan(
                          text:
                              "Save it somewhere safe (not on this phone). If you lose your seed and your phone, you've lost your funds."),
                    ])),
              ),
            ),
            const SizedBox(height: 10),
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.center,
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
          ],
        ),
      )),
    );
  }
}
