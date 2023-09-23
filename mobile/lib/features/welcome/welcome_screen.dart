import 'dart:io';

import 'package:f_logs/model/flog/flog.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/ffi.dart';
import 'package:url_launcher/url_launcher.dart';

class WelcomeScreen extends StatefulWidget {
  static const route = "/welcome";
  static const label = "Welcome";

  const WelcomeScreen({Key? key}) : super(key: key);

  @override
  State<WelcomeScreen> createState() => _WelcomeScreenState();
}

class _WelcomeScreenState extends State<WelcomeScreen> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();

  String _email = "";

  /// TODO Convert to a flutter package that checks the email domain validity
  /// (MX record, etc.)
  bool isEmailValid(String email) {
    return RegExp(r"^[a-zA-Z0-9.a-zA-Z0-9.!#$%&'*+-/=?^_`{|}~]+@[a-zA-Z0-9]+\.[a-zA-Z]+")
        .hasMatch(email);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        appBar: AppBar(title: const Text("Welcome to 10101 beta.")),
        body: ScrollableSafeArea(
            child: Form(
          key: _formKey,
          child: Container(
            color: Colors.white,
            padding: const EdgeInsets.all(20.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: <Widget>[
                Center(
                  child: Image.asset('assets/10101_logo_icon.png', width: 150, height: 150),
                ),
                const Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Center(
                        child: Text(
                      "As we are in closed beta, there may be bugs. To assist with any issues, please provide your email.",
                      style: TextStyle(fontSize: 16, color: Colors.black54),
                    ))
                  ],
                ),
                const SizedBox(height: 20),
                TextFormField(
                  keyboardType: TextInputType.emailAddress,
                  initialValue: _email,
                  decoration: const InputDecoration(
                    labelText: 'Email',
                    hintText: 'Enter your email address to continue',
                  ),
                  validator: (value) {
                    if (value == null || value.isEmpty || !isEmailValid(value)) {
                      return 'Please enter a valid email address';
                    }
                    return null;
                  },
                  onSaved: (value) {
                    _email = value ?? "";
                  },
                ),
                ElevatedButton(
                  onPressed: () async {
                    if (_formKey.currentState != null && _formKey.currentState!.validate()) {
                      _formKey.currentState?.save();
                      try {
                        final currentContext = context;
                        await api.registerBeta(email: _email).then((value) {
                          FLog.info(text: "Successfully registered email $_email for beta access.");
                          Preferences.instance.setEmailAddress(_email);
                          currentContext.go(WalletScreen.route);
                        });
                      } catch (e) {
                        FLog.error(text: e.toString());
                        showDialog(
                            context: rootNavigatorKey.currentContext!,
                            builder: (context) => AlertDialog(
                                    title: const Text("No access to the 10101 beta."),
                                    content: RichText(
                                      text: TextSpan(
                                        style: DefaultTextStyle.of(context).style,
                                        children: <TextSpan>[
                                          const TextSpan(
                                              text: "Worry not, you are on the wait-list.\n"
                                                  "Please be patient as we scale up our infrastructure.\n If you have further questions, please contact us on Telegram: ",
                                              style:
                                                  TextStyle(fontSize: 16, color: Colors.black54)),
                                          TextSpan(
                                            text: 'https://t.me/get10101',
                                            style:
                                                const TextStyle(fontSize: 16, color: Colors.blue),
                                            recognizer: TapGestureRecognizer()
                                              ..onTap = () async {
                                                final httpsUri = Uri(
                                                    scheme: 'https',
                                                    host: 't.me',
                                                    path: 'get10101');
                                                if (await canLaunchUrl(httpsUri)) {
                                                  await launchUrl(httpsUri);
                                                } else {
                                                  showSnackBar(
                                                      ScaffoldMessenger.of(
                                                          rootNavigatorKey.currentContext!),
                                                      "Failed to open link");
                                                }
                                              },
                                          ),
                                        ],
                                      ),
                                    ),
                                    actions: [
                                      TextButton(
                                        onPressed: () => exit(0),
                                        child: const Text('OK'),
                                      ),
                                    ]));
                      }
                    }
                  },
                  child: const Text(
                    'Continue',
                    style: TextStyle(fontSize: 16),
                  ),
                ),
              ],
            ),
          ),
        )));
  }

  @override
  void initState() {
    super.initState();

    Preferences.instance.getEmailAddress().then((value) => setState(() {
          _email = value;
          FLog.info(text: "retrieved stored email from the preferences: $_email.");
        }));
  }
}
