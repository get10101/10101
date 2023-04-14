import 'package:f_logs/model/flog/flog.dart';
import 'package:flutter/material.dart';
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
        appBar: AppBar(title: const Text("Welcome to 10101 beta!")),
        body: SafeArea(
            child: Form(
          key: _formKey,
          child: Column(
            children: <Widget>[
              Column(
                children: const <Widget>[
                  Text("Please be patient with us as we work out the rough edges."),
                  Text("Any feedback is welcome!"),
                  SizedBox(height: 10),
                  Text("Please enter your email address to continue:"),
                ],
              ),
              const SizedBox(height: 20),
              TextFormField(
                keyboardType: TextInputType.emailAddress,
                initialValue: _email,
                decoration: const InputDecoration(
                  labelText: 'Email',
                  hintText: 'Enter your email address',
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
                onPressed: () {
                  if (_formKey.currentState != null && _formKey.currentState!.validate()) {
                    _formKey.currentState?.save();
                    Preferences.instance.setEmailAddress(_email);
                    FLog.info(text: "Successfully stored the email address $_email .");
                    api.registerBeta(email: _email);
                    context.go(WalletScreen.route);
                  }
                },
                child: const Text('Start'),
              ),
            ],
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
