import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:share_plus/share_plus.dart';

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
        padding: const EdgeInsets.only(top: 20, left: 18, right: 18),
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
                future: rust.api.referralStatus(),
                builder: (BuildContext context, AsyncSnapshot<rust.ReferralStatus> snapshot) {
                  if (!snapshot.hasData) {
                    return const CircularProgressIndicator();
                  }

                  final referralCode = snapshot.data!.referralCode;
                  final referralTier = snapshot.data!.referralTier;
                  final numberOfActivatedReferrals = snapshot.data!.numberOfActivatedReferrals;
                  final numberOfTotalReferrals = snapshot.data!.numberOfTotalReferrals;
                  final referralFeeBonus = snapshot.data!.referralFeeBonus.toStringAsFixed(2);

                  return Column(
                    children: [
                      const Text(
                        "Referral status",
                        style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                      ),
                      const SizedBox(
                        height: 5,
                      ),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          const Text("Referral Code"),
                          Row(
                            children: [
                              SelectableText(referralCode),
                              const SizedBox(
                                width: 10,
                              ),
                              GestureDetector(
                                onTap: () async {
                                  showSnackBar(
                                      ScaffoldMessenger.of(context), "Copied $referralCode");
                                  await Clipboard.setData(ClipboardData(text: referralCode));
                                },
                                child: Icon(
                                  Icons.copy,
                                  size: 17,
                                  color: tenTenOnePurple.shade800,
                                ),
                              ),
                              const SizedBox(
                                width: 10,
                              ),
                              GestureDetector(
                                child: const Icon(Icons.share, size: 16),
                                onTap: () => Share.share(
                                    "Join me and trade without counter-party risk. Use this referral to get a fee discount: $referralCode"),
                              )
                            ],
                          )
                        ],
                      ),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          const Text("Active referrals"),
                          Text(numberOfActivatedReferrals.toString()),
                        ],
                      ),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          const Text("Total referrals"),
                          Row(
                            children: [
                              Text("$numberOfTotalReferrals"),
                              const Text('*',
                                  style: TextStyle(
                                    fontFeatures: <FontFeature>[
                                      FontFeature.superscripts(),
                                    ],
                                  )),
                            ],
                          ),
                        ],
                      ),
                      const Divider(),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          const Text("Referral Tier"),
                          Text(referralTier.toString()),
                        ],
                      ),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          const Text("Active referral bonus"),
                          Text("$referralFeeBonus%"),
                        ],
                      ),
                      const SizedBox(
                        height: 10,
                      ),
                      const Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          Text('*',
                              style: TextStyle(
                                fontFeatures: <FontFeature>[
                                  FontFeature.superscripts(),
                                ],
                              )),
                          Text("these users have not yet traded enough.")
                        ],
                      )
                    ],
                  );
                }),
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

                  return Column(
                    children: [
                      const Row(
                        children: [
                          Text(
                            "Settings",
                            style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                          ),
                        ],
                      ),
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
