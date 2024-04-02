import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/ffi.dart' as rust;

class UserSettings extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "user";

  const UserSettings({super.key});

  @override
  State<UserSettings> createState() => _UserSettingsState();
}

class _UserSettingsState extends State<UserSettings> {
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
                            "User Settings",
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
            FutureBuilder(
                future: rust.api.getUserDetails(),
                builder: (BuildContext context, AsyncSnapshot<rust.User> snapshot) {
                  if (!snapshot.hasData) {
                    return const CircularProgressIndicator();
                  }

                  final nickname = snapshot.data?.nickname;
                  final contact = snapshot.data?.contact;

                  return Expanded(
                    child: Padding(
                      padding: const EdgeInsets.all(8.0),
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.start,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          CustomInputField(
                            onConfirm: (value) async {
                              final messenger = ScaffoldMessenger.of(context);
                              try {
                                await rust.api.registerBeta(contact: value);
                                showSnackBar(messenger, "Successfully updated to `$value`");
                              } catch (error) {
                                showSnackBar(messenger, "Failed updating details due to $error");
                              }
                            },
                            labelText: "Contact details",
                            initialValue: contact,
                            hintText: "Nostr, Email, X-handle",
                          ),
                          const SizedBox(
                            height: 20,
                          ),
                          NicknameWidget(
                            initialValue: nickname,
                            labelText: 'Nickname',
                            onConfirm: (value) async {
                              final messenger = ScaffoldMessenger.of(context);
                              try {
                                await rust.api.updateNickname(nickname: value);
                                showSnackBar(messenger, "Successfully updated to `$value`");
                              } catch (error) {
                                showSnackBar(messenger, "Failed updating details due to $error");
                              }
                            },
                          )
                        ],
                      ),
                    ),
                  );
                }),
          ],
        ),
      )),
    );
  }
}

class NicknameWidget extends StatefulWidget {
  final String labelText;
  final String? initialValue;
  final Function onConfirm;

  const NicknameWidget({
    super.key,
    required this.onConfirm,
    required this.labelText,
    this.initialValue,
  });

  @override
  State<NicknameWidget> createState() => _NicknameWidgetState();
}

class _NicknameWidgetState extends State<NicknameWidget> {
  bool fieldEnabled = false;
  late final String labelText;
  late final Function onConfirm;
  TextEditingController fieldController = TextEditingController();

  @override
  void initState() {
    super.initState();
    labelText = widget.labelText;
    onConfirm = widget.onConfirm;
    if (widget.initialValue != null) {
      fieldController.text = widget.initialValue!;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      alignment: Alignment.centerRight,
      children: [
        TextFormField(
          enabled: fieldEnabled,
          controller: fieldController,
          decoration: InputDecoration(
            border: const UnderlineInputBorder(),
            labelText: labelText,
          ),
        ),
        Visibility(
          replacement: IconButton(
            icon: const Icon(Icons.edit),
            onPressed: () {
              setState(() {
                fieldEnabled = true;
              });
            },
          ),
          visible: fieldEnabled,
          child: Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              IconButton(
                icon: const Icon(
                  Icons.refresh,
                  color: tenTenOnePurple,
                ),
                onPressed: () {
                  final newRandomName = rust.api.getNewRandomName();
                  setState(() {
                    fieldController.text = newRandomName;
                  });
                },
              ),
              IconButton(
                icon: const Icon(
                  Icons.check,
                  color: Colors.green,
                ),
                onPressed: () async {
                  final newContact = fieldController.value.text;
                  try {
                    await onConfirm(newContact);
                  } finally {
                    setState(() {
                      fieldEnabled = false;
                    });
                  }
                },
              ),
            ],
          ),
        )
      ],
    );
  }

  void showSnackBar(ScaffoldMessengerState messenger, String message) {
    messenger.showSnackBar(SnackBar(content: Text(message)));
  }
}

class CustomInputField extends StatefulWidget {
  final String labelText;
  final String? hintText;
  final String? initialValue;
  final Function onConfirm;

  const CustomInputField({
    super.key,
    required this.onConfirm,
    required this.labelText,
    this.hintText,
    this.initialValue,
  });

  @override
  State<CustomInputField> createState() => _CustomInputFieldState();
}

class _CustomInputFieldState extends State<CustomInputField> {
  bool fieldEnabled = false;
  late final String labelText;
  late final Function onConfirm;
  TextEditingController fieldController = TextEditingController();

  @override
  void initState() {
    super.initState();
    labelText = widget.labelText;
    onConfirm = widget.onConfirm;
    if (widget.initialValue != null) {
      fieldController.text = widget.initialValue!;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      alignment: Alignment.centerRight,
      children: [
        TextFormField(
          enabled: fieldEnabled,
          controller: fieldController,
          decoration: InputDecoration(
            border: const UnderlineInputBorder(),
            labelText: labelText,
            hintText: widget.hintText,
          ),
        ),
        Visibility(
          replacement: IconButton(
            icon: const Icon(Icons.edit),
            onPressed: () {
              setState(() {
                fieldEnabled = true;
              });
            },
          ),
          visible: fieldEnabled,
          child: IconButton(
            icon: const Icon(
              Icons.check,
              color: Colors.green,
            ),
            onPressed: () async {
              final newContact = fieldController.value.text;
              try {
                await onConfirm(newContact);
              } finally {
                setState(() {
                  fieldEnabled = false;
                });
              }
            },
          ),
        )
      ],
    );
  }
}
