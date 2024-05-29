import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/ffi.dart';
import 'package:get_10101/util/poll_change_notified.dart';
import 'package:provider/provider.dart';

class PollWidget extends StatefulWidget {
  const PollWidget({super.key});

  @override
  State<PollWidget> createState() => _PollWidgetState();
}

class _PollWidgetState extends State<PollWidget> {
  Choice? _selectedAnswer;
  bool showPoll = true;

  @override
  Widget build(BuildContext context) {
    final PollChangeNotifier pollChangeNotifier = context.watch<PollChangeNotifier>();
    var poll = pollChangeNotifier.poll;
    if (poll == null || !showPoll) {
      return const SizedBox.shrink();
    }

    return Card(
        margin: const EdgeInsets.all(0),
        elevation: 1,
        child: Stack(
          children: [
            ExpansionTile(
              backgroundColor: Colors.white,
              trailing: const SizedBox.shrink(),
              title: const Text("Time for a quick survey?"),
              children: [
                Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Stack(children: [
                      Align(
                        alignment: Alignment.topCenter,
                        child: Padding(
                          padding: const EdgeInsets.only(top: 10.0, bottom: 10),
                          child: RichText(
                              text: TextSpan(
                            text: poll.question,
                            style: const TextStyle(
                                fontWeight: FontWeight.w500, color: Colors.black, fontSize: 20),
                          )),
                        ),
                      )
                    ]),
                    Column(
                        children: poll.choices
                            .map(
                              (choice) => PollChoice(
                                padding: const EdgeInsets.symmetric(horizontal: 5.0),
                                value: choice,
                                groupValue: _selectedAnswer,
                                onChanged: (Choice answer) {
                                  setState(() {
                                    _selectedAnswer = answer;
                                  });
                                },
                                selected: _selectedAnswer?.id == choice.id,
                              ),
                            )
                            .toList()),
                    Padding(
                      padding: const EdgeInsets.all(8.0),
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.end,
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          TextButton(
                            onPressed: () async {
                              final messenger = ScaffoldMessenger.of(context);
                              await pollChangeNotifier.ignore(poll);
                              showSnackBar(messenger, "Poll won't be shown again");
                              await pollChangeNotifier.refresh();
                            },
                            style: ButtonStyle(
                              padding:
                                  WidgetStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                              backgroundColor: WidgetStateProperty.all<Color>(Colors.white),
                            ),
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.start,
                              children: [
                                RichText(
                                  text: const TextSpan(
                                    text: "Ignore this poll",
                                    style: TextStyle(
                                      fontWeight: FontWeight.w500,
                                      color: Colors.black,
                                      fontSize: 15,
                                      decoration: TextDecoration.underline,
                                    ),
                                  ),
                                ),
                                const SizedBox(width: 5),
                                const Icon(
                                  Icons.cancel,
                                  size: 16,
                                  color: Colors.black,
                                ),
                              ],
                            ),
                          ),
                          ElevatedButton(
                            onPressed: () async {
                              final messenger = ScaffoldMessenger.of(context);
                              if (_selectedAnswer == null) {
                                showSnackBar(messenger, "Please provide an answer");
                              } else {
                                await answerPoll(pollChangeNotifier, poll, _selectedAnswer!);
                              }
                            },
                            style: ElevatedButton.styleFrom(
                              padding: const EdgeInsets.all(8.0),
                              backgroundColor: Colors.grey.shade200,
                            ),
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.end,
                              children: [
                                RichText(
                                  text: const TextSpan(
                                    text: "Submit",
                                    style: TextStyle(
                                      fontWeight: FontWeight.w500,
                                      color: Colors.black,
                                      fontSize: 15,
                                    ),
                                  ),
                                ),
                                const SizedBox(width: 5),
                                const Icon(
                                  Icons.send,
                                  size: 16,
                                  color: Colors.black,
                                ),
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                )
              ],
            ),
            Align(
                alignment: Alignment.topRight,
                child: Padding(
                  padding: const EdgeInsets.all(8.0),
                  child: IconButton(
                    onPressed: () => setState(() {
                      showPoll = false;
                    }),
                    icon: const Icon(
                      Icons.close,
                      size: 28,
                    ),
                  ),
                )),
          ],
        ));
  }

  Future<void> answerPoll(
      PollChangeNotifier pollChangeNotifier, Poll poll, Choice selectedAnswer) async {
    await pollChangeNotifier.answer(selectedAnswer, poll);
  }
}

class PollChoice extends StatelessWidget {
  final EdgeInsets padding;
  final Choice? groupValue;
  final Choice value;
  final ValueChanged<Choice> onChanged;
  final bool selected;

  const PollChoice({
    super.key,
    required this.padding,
    required this.groupValue,
    required this.value,
    required this.onChanged,
    required this.selected,
  });

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: () {
        if (value != groupValue) {
          onChanged(value);
        }
      },
      child: Padding(
        padding: padding,
        child: Column(
          children: [
            Row(
              children: <Widget>[
                Radio<int>(
                  groupValue: groupValue?.id,
                  value: value.id,
                  onChanged: (int? newValue) {
                    onChanged(value);
                  },
                ),
                value.editable
                    ? Expanded(
                        child: TextField(
                          enabled: selected,
                          onChanged: (answer) =>
                              {onChanged(Choice(id: value.id, value: answer, editable: true))},
                          decoration: InputDecoration(
                            hintText: value.value,
                          ),
                        ),
                      )
                    : Text(value.value),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
