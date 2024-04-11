import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:path_provider/path_provider.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:screenshot/screenshot.dart';
import 'package:share_plus/share_plus.dart';
import 'package:get_10101/ffi.dart' as rust;

class ShareReferralWidget extends StatefulWidget {
  const ShareReferralWidget({
    super.key,
    required this.screenShotController,
    required this.pref,
  });

  final ScreenshotController screenShotController;
  final Preferences pref;

  @override
  State<ShareReferralWidget> createState() => _ShareReferralWidgetState();
}

class _ShareReferralWidgetState extends State<ShareReferralWidget> {
  @override
  Widget build(BuildContext context) {
    return FutureBuilder(
      future: rust.api.referralStatus(),
      builder: (BuildContext context, AsyncSnapshot<rust.ReferralStatus> snapshot) {
        if (!snapshot.hasData) {
          return const CircularProgressIndicator();
        }

        final referralStatus = snapshot.data!;

        return AlertDialog(
          title: const Text(
            "Enjoying 10101?",
            textAlign: TextAlign.center,
          ),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.start,
            children: [
              Center(
                child: RichText(
                  textAlign: TextAlign.center,
                  text: const TextSpan(
                    text: "Invite your friends to join ",
                    children: [
                      TextSpan(
                          text: "10101",
                          style: TextStyle(fontWeight: FontWeight.bold, color: tenTenOnePurple),
                          children: [
                            TextSpan(
                                text: " and we’ll reduce your order matching fee.",
                                style:
                                    TextStyle(fontWeight: FontWeight.normal, color: Colors.black))
                          ])
                    ],
                    style: TextStyle(
                      fontSize: 18,
                      color: Colors.black,
                    ),
                  ),
                ),
              ),
              const SizedBox(
                height: 10,
              ),
              const Text("Your referral code is "),
              const SizedBox(
                height: 10,
              ),
              Stack(
                children: [
                  SizedBox(
                    height: 50,
                    child: Center(
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          RichText(
                            text: TextSpan(
                                text: referralStatus.referralCode,
                                style: const TextStyle(
                                  fontWeight: FontWeight.bold,
                                  color: tenTenOnePurple,
                                  fontSize: 18,
                                ),
                                recognizer: TapGestureRecognizer()
                                  ..onTap = () {
                                    Clipboard.setData(
                                        ClipboardData(text: referralStatus.referralCode));
                                    ScaffoldMessenger.of(context).showSnackBar(
                                      const SnackBar(
                                        content: Text('Copied to clipboard'),
                                      ),
                                    );
                                  }),
                          ),
                        ],
                      ),
                    ),
                  ),
                  SizedBox(
                    height: 50,
                    child: Center(
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          IconButton(
                              onPressed: () {
                                Share.share(
                                    "Join me and trade without counter-party risk. Use this referral to get a fee discount: ${referralStatus.referralCode}");
                              },
                              icon: const Icon(Icons.share, size: 18))
                        ],
                      ),
                    ),
                  ),
                ],
              ),
              Screenshot(
                  controller: widget.screenShotController,
                  child: FittedBox(
                    fit: BoxFit.scaleDown,
                    child: SizedBox(
                      width: 600,
                      height: 800,
                      child: ReferralWidget(
                        referralStatus: referralStatus,
                      ),
                    ),
                  ))
            ],
          ),
          actions: <Widget>[
            TextButton(
              onPressed: () {
                widget.pref.storeDontShowReferralDialogFor7Days();
                Navigator.pop(context, 'Cancel');
              },
              child: const Text(
                'Not now',
                style: TextStyle(
                  decoration: TextDecoration.underline,
                ),
              ),
            ),
            ElevatedButton(
              child: const Text("Share"),
              onPressed: () async {
                await widget.screenShotController
                    .capture(delay: const Duration(milliseconds: 10))
                    .then((image) async {
                  if (image != null) {
                    final directory = await getApplicationDocumentsDirectory();
                    final imagePath = await File('${directory.path}/join-the-future.png').create();
                    await imagePath.writeAsBytes(image);
                    await Share.shareXFiles(
                        [XFile(imagePath.path, mimeType: "image/x-png", bytes: image)]);
                  }
                }).catchError((error) {
                  logger.e("Failed at capturing screenshot", error: error);
                }).whenComplete(() {
                  widget.pref.storeDontShowReferralDialogFor7Days();
                  Navigator.of(context).pop(); // Close the dialog
                });
              },
            )
          ],
        );
      },
    );
  }
}

class ReferralWidget extends StatelessWidget {
  final rust.ReferralStatus referralStatus;

  const ReferralWidget({
    Key? key,
    required this.referralStatus,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: <Widget>[
        SizedBox(
          width: 600,
          height: 800,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(30.0),
            child: Image.asset("assets/referral_10101_laser_eyes.png"),
          ),
        ),
        Align(
          alignment: Alignment.topRight,
          child: Padding(
            padding: const EdgeInsets.all(20.0),
            child: SizedBox(
                width: 150,
                height: 150,
                child: Padding(
                  padding: const EdgeInsets.all(10.0),
                  child: Container(
                      decoration: const BoxDecoration(
                        color: Colors.white,
                      ),
                      child: QrImageView(
                        data:
                            "https://referral.10101.finance?referral=${referralStatus.referralCode}",
                        eyeStyle: const QrEyeStyle(
                          eyeShape: QrEyeShape.square,
                          color: Colors.black,
                        ),
                        dataModuleStyle: const QrDataModuleStyle(
                          dataModuleShape: QrDataModuleShape.square,
                          color: Colors.black,
                        ),
                        version: QrVersions.auto,
                        padding: const EdgeInsets.all(1),
                      )),
                )),
          ),
        ),
        Container(
          alignment: Alignment.center,
          child: Column(
            children: [
              const Spacer(flex: 2),
              SizedBox(
                height: 247,
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    const Padding(
                      padding: EdgeInsets.only(left: 20, right: 20),
                      child: Text(
                        "Join 10101 and trade without counterparty risk.",
                        style: TextStyle(fontSize: 22, color: Colors.white),
                        textAlign: TextAlign.center,
                      ),
                    ),
                    const Padding(
                      padding: EdgeInsets.only(left: 20, right: 20),
                      child: Text(
                        "The future of finance is here!",
                        style: TextStyle(fontSize: 22, color: Colors.white),
                        textAlign: TextAlign.center,
                      ),
                    ),
                    const SizedBox(
                      height: 20,
                    ),
                    Text(
                      referralStatus.referralCode,
                      style: const TextStyle(
                          fontSize: 30, color: Colors.white, fontWeight: FontWeight.bold),
                      textAlign: TextAlign.center,
                    ),
                    const SizedBox(
                      height: 20,
                    ),
                    const Padding(
                      padding: EdgeInsets.only(left: 20, right: 20),
                      child: Text(
                        "Use my referral code to reduce your trading fee",
                        style: TextStyle(fontSize: 22, color: Colors.white),
                        textAlign: TextAlign.center,
                      ),
                    ),
                    const SizedBox(
                      height: 20,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
