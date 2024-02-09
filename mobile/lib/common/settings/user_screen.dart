import 'package:flutter/material.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/logger/logger.dart';
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
  var contactFieldController = TextEditingController();
  bool contactFieldEnabled = false;

  @override
  void initState() {
    super.initState();
    rust.api
        .getUserDetails()
        .then((user) => contactFieldController.text = user.contact != null ? user.contact! : "");
  }

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
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.start,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    "Contact details \n",
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                  ),
                  RichText(
                    text: const TextSpan(
                      text:
                          '10101 will use these details to reach out to you in case of problems in the app. \n\n',
                      style: TextStyle(fontSize: 16, color: Colors.black),
                      children: <TextSpan>[
                        TextSpan(
                          text: 'This can be a ',
                        ),
                        TextSpan(
                            text: 'Nostr Pubkey', style: TextStyle(fontWeight: FontWeight.bold)),
                        TextSpan(
                          text: ', a ',
                        ),
                        TextSpan(
                            text: 'Telegram handle ',
                            style: TextStyle(fontWeight: FontWeight.bold)),
                        TextSpan(
                          text: 'or an ',
                        ),
                        TextSpan(
                            text: 'email address.', style: TextStyle(fontWeight: FontWeight.bold)),
                        TextSpan(
                            text:
                                "\n\nIf you want to delete your contact details. Simply remove the details below.")
                      ],
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 16),
                    child: Stack(
                      alignment: Alignment.centerRight,
                      children: [
                        TextFormField(
                          enabled: contactFieldEnabled,
                          controller: contactFieldController,
                          decoration: const InputDecoration(
                            border: UnderlineInputBorder(),
                            labelText: 'Contact details',
                          ),
                        ),
                        Visibility(
                          replacement: IconButton(
                            icon: const Icon(Icons.edit),
                            onPressed: () {
                              setState(() {
                                contactFieldEnabled = true;
                              });
                            },
                          ),
                          visible: contactFieldEnabled,
                          child: IconButton(
                            icon: const Icon(
                              Icons.check,
                              color: Colors.green,
                            ),
                            onPressed: () async {
                              final messenger = ScaffoldMessenger.of(context);
                              try {
                                var newContact = contactFieldController.value.text;
                                logger.i("Successfully updated to $newContact");
                                await rust.api.registerBeta(contact: newContact);
                                showSnackBar(messenger, "Successfully updated to $newContact");
                              } catch (exception) {
                                showSnackBar(
                                    messenger, "Error when updating contact details $exception");
                              } finally {
                                setState(() {
                                  contactFieldEnabled = false;
                                });
                              }
                            },
                          ),
                        )
                      ],
                    ),
                  )
                ],
              ),
            ),
          ],
        ),
      )),
    );
  }
}
