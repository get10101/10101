import 'package:flutter/material.dart';
import 'package:get_10101/settings/seed_words.dart';
import 'package:get_10101/settings/settings_service.dart';
import 'package:provider/provider.dart';

class SeedScreen extends StatefulWidget {
  const SeedScreen({super.key});

  @override
  State<SeedScreen> createState() => _SeedScreenState();
}

class _SeedScreenState extends State<SeedScreen> {
  bool checked = false;
  bool visibility = false;

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final settingsService = context.read<SettingsService>();
    return FutureBuilder(
      future: settingsService.getSeedPhrase(),
      builder: (BuildContext context, AsyncSnapshot<List<String>> snapshot) {
        if (!snapshot.hasData) {
          return const CircularProgressIndicator();
        }
        if (snapshot.hasData) {
          List<String>? phrase = snapshot.data!;
          return Scaffold(
            body: Padding(
              padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
              child: Column(
                children: [
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
                                    "Beware of phishing. The developers of 10101 will never ask for your seed.\n\n"),
                            TextSpan(
                                text: "Do not lose this seed. ",
                                style: TextStyle(fontWeight: FontWeight.bold)),
                            TextSpan(
                                text:
                                    "Save it somewhere safe. If you lose your seed your lose your funds."),
                          ])),
                    ),
                  ),
                  const SizedBox(height: 25),
                  Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      buildSeedWordsWidget(phrase, visibility),
                      const SizedBox(height: 20),
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
                ],
              ),
            ),
          );
        }
        return const SizedBox.shrink();
      },
    );
  }
}
