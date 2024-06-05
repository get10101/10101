import 'package:flutter/material.dart';

Row buildSeedWordsWidget(List<String> phrase, bool visible) {
  final firstColumn = phrase
      .getRange(0, 6)
      .toList()
      .asMap()
      .entries
      .map((entry) => SeedWord(entry.value, entry.key + 1, visible))
      .toList();
  final secondColumn = phrase
      .getRange(6, 12)
      .toList()
      .asMap()
      .entries
      .map((entry) => SeedWord(entry.value, entry.key + 7, visible))
      .toList();

  return Row(
    mainAxisAlignment: MainAxisAlignment.center,
    crossAxisAlignment: CrossAxisAlignment.center,
    children: [
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: firstColumn),
      const SizedBox(width: 30),
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: secondColumn)
    ],
  );
}

class SeedWord extends StatelessWidget {
  final String? word;
  final int? index;
  final bool visibility;

  const SeedWord(this.word, this.index, this.visibility, {super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
        padding: const EdgeInsets.fromLTRB(0, 5, 0, 5),
        child: Row(
            crossAxisAlignment: visibility ? CrossAxisAlignment.baseline : CrossAxisAlignment.end,
            textBaseline: TextBaseline.alphabetic,
            children: [
              SizedBox(
                width: 30.0,
                child: Text(
                  '#$index',
                  style: TextStyle(fontSize: 14, color: Colors.grey[600]),
                ),
              ),
              const SizedBox(width: 5),
              visibility
                  ? SizedBox(
                      width: 100,
                      child: Text(
                        word!,
                        style: const TextStyle(fontSize: 20, fontWeight: FontWeight.bold),
                      ),
                    )
                  : Container(width: 100, height: 24, color: Colors.grey[300]),
            ]));
  }
}
