import 'package:get_10101/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/ffi.dart';

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
        body: ScrollableSafeArea(
            child: Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        mainAxisAlignment: MainAxisAlignment.center,
        children: <Widget>[
          const Spacer(),
          Center(
            child: Image.asset('assets/10101_logo_icon.png', width: 150, height: 150),
          ),
          const Spacer(),
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Container(
                  padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
                  child: const Text(
                    "As we are in closed beta, there may be bugs. To assist with any issues, please provide your email.",
                    style: TextStyle(fontSize: 16, color: Colors.black54),
                  ))
            ],
          ),
          Container(
            padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
            child: TextFormField(
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
          ),
          ElevatedButton(
            onPressed: () {
              if (_formKey.currentState != null && _formKey.currentState!.validate()) {
                _formKey.currentState?.save();
                try {
                  api.registerBeta(email: _email);
                  Preferences.instance.setEmailAddress(_email);
                  logger.i("Successfully stored the email address $_email .");
                  context.go(WalletScreen.route);
                } catch (e) {
                  showSnackBar(ScaffoldMessenger.of(context), "$e");
                }
              }
            },
            child: const Text(
              'Continue',
              style: TextStyle(fontSize: 16),
            ),
          ),
          const Spacer(),
        ],
      ),
    )));
  }

  @override
  void initState() {
    super.initState();

    Preferences.instance.getEmailAddress().then((value) => setState(() {
          _email = value;
          logger.i("retrieved stored email from the preferences: $_email.");
        }));
  }
}
