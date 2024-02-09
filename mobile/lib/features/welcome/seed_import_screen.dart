import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/welcome/loading_screen.dart';
import 'package:get_10101/features/welcome/onboarding.dart';
import 'package:get_10101/ffi.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/file.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:go_router/go_router.dart';

class SeedPhraseImporter extends StatefulWidget {
  static const route = "${Onboarding.route}/$subRouteName";
  static const subRouteName = "seed-importer";

  const SeedPhraseImporter({Key? key}) : super(key: key);

  @override
  SeedPhraseImporterState createState() => SeedPhraseImporterState();
}

class SeedPhraseImporterState extends State<SeedPhraseImporter> {
  late TextEditingController _controller;
  late List<String> twelveWords;
  late FocusNode focusNode;

  @override
  void initState() {
    twelveWords = [];
    _controller = TextEditingController();
    focusNode = FocusNode();
    super.initState();
  }

  @override
  void dispose() {
    _controller.dispose();
    twelveWords.clear();
    focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    var isImportDisabled = twelveWords.length < 12;

    return Scaffold(
      appBar: AppBar(title: const Text('Manual restore')),
      // this is needed to prevent an overflow when the keyboard is up
      resizeToAvoidBottomInset: false,
      body: Column(
        mainAxisAlignment: MainAxisAlignment.start,
        children: <Widget>[
          const Padding(
            padding: EdgeInsets.all(16.0),
            child: Text(
              "Please enter your seed phrase.",
              style: TextStyle(fontSize: 20),
            ),
          ),
          SizedBox(
            child: Padding(
              padding: const EdgeInsets.all(16.0),
              child: TextField(
                autofocus: true,
                focusNode: focusNode,
                controller: _controller,
                enabled: twelveWords.length < 12,
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: 'Enter keywords from your seed',
                ),
                onSubmitted: (String value) async {
                  if (value.isNotEmpty) {
                    setState(() {
                      var split = value.split(" ");
                      twelveWords.addAll(split);
                      _controller.clear();
                      focusNode.requestFocus();
                    });
                  }
                },
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(16.0, 16.0, 16.0, 0),
            child: WordTable(words: twelveWords),
          ),
          Padding(
              padding: const EdgeInsets.fromLTRB(16.0, 16.0, 16.0, 32),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                children: [
                  SizedBox(
                    width: 120,
                    child: ElevatedButton(
                      style: ButtonStyle(
                        padding: MaterialStateProperty.all<EdgeInsets>(
                            const EdgeInsets.fromLTRB(20, 10, 20, 10)),
                      ),
                      onPressed: () {
                        setState(() {
                          twelveWords.clear();
                          _controller.clear();
                        });
                      },
                      child: const Text(
                        "Clear",
                        style: TextStyle(fontSize: 20),
                      ),
                    ),
                  ),
                  SizedBox(
                    width: 120,
                    child: ElevatedButton(
                      style: ButtonStyle(
                        padding: MaterialStateProperty.all<EdgeInsets>(
                            const EdgeInsets.fromLTRB(20, 10, 20, 10)),
                        backgroundColor:
                            MaterialStateProperty.resolveWith<Color>((Set<MaterialState> states) {
                          if (states.contains(MaterialState.disabled)) {
                            return Colors.grey; // Change to the desired disabled color
                          }
                          return tenTenOnePurple; // Change to the desired enabled color
                        }),
                        foregroundColor:
                            MaterialStateProperty.resolveWith<Color>((Set<MaterialState> states) {
                          if (states.contains(MaterialState.disabled)) {
                            return Colors.black; // Change to the desired disabled text color
                          }
                          return Colors.white; // Change to the desired enabled text color
                        }),
                      ),
                      onPressed: isImportDisabled
                          ? null
                          : () async {
                              logger.i("Restoring a previous wallet from a given seed phrase");
                              final seedPhrase = twelveWords.join(" ");
                              getSeedFilePath().then((seedPath) {
                                logger.i("Restoring seed into $seedPath");

                                final restore = api
                                    .restoreFromSeedPhrase(
                                        seedPhrase: seedPhrase, targetSeedFilePath: seedPath)
                                    .catchError((error) => showSnackBar(
                                        ScaffoldMessenger.of(context),
                                        "Failed to import from seed. $error"));
                                // TODO(holzeis): Backup preferences and restore email from there.
                                Preferences.instance.setContactDetails("restored");
                                GoRouter.of(context).go(LoadingScreen.route, extra: restore);
                              }).catchError((e) {
                                logger.e("Error restoring from seed phrase: $e");
                                showSnackBar(ScaffoldMessenger.of(rootNavigatorKey.currentContext!),
                                    "Failed to restore seed: $e");
                              });
                            },
                      child: const Text(
                        "Import",
                        style: TextStyle(fontSize: 20),
                      ),
                    ),
                  ),
                ],
              )),
        ],
      ),
    );
  }
}

class WordTable extends StatelessWidget {
  final List<String> words;

  const WordTable({super.key, required this.words});

  @override
  Widget build(BuildContext context) {
    return Table(
      children: List.generate(6, (row) {
        return TableRow(
          children: List.generate(2, (col) {
            return TableCell(
              child: Padding(
                padding: const EdgeInsets.all(8.0),
                child: RichText(
                  text: TextSpan(
                    children: [
                      TextSpan(
                        text: '#${calculateIndex(col, row) + 1} ',
                        style: TextStyle(color: tenTenOnePurple.shade300, fontSize: 20),
                      ),
                      TextSpan(
                        text: getWord(calculateIndex(col, row), words),
                        style: const TextStyle(color: Colors.black, fontSize: 20),
                      ),
                    ],
                  ),
                ),
              ),
            );
          }),
        );
      }),
    );
  }
}

int calculateIndex(int col, int row) => col == 0 ? row : row + 6;

String getWord(int index, List<String> words) {
  if (index >= 0 && index < words.length) {
    var word = words[index];
    return word;
  } else {
    return '';
  }
}
