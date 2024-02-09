import 'package:flutter/services.dart';
import 'package:get_10101/common/application/switch.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/features/welcome/loading_screen.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/util/file.dart';
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
  bool _betaDisclaimer = false;
  bool _loseDisclaimer = false;

  /// TODO Convert to a flutter package that checks the email domain validity
  /// (MX record, etc.)
  bool isEmailValid(String email) {
    return RegExp(r"^[a-zA-Z0-9.a-zA-Z0-9.!#$%&'*+-/=?^_`{|}~]+@[a-zA-Z0-9]+\.[a-zA-Z]+")
        .hasMatch(email);
  }

  @override
  Widget build(BuildContext context) {
    return AnnotatedRegion<SystemUiOverlayStyle>(
        value: SystemUiOverlayStyle.dark,
        child: Scaffold(
          backgroundColor: Colors.white,
          body: SafeArea(
            bottom: false,
            child: Container(
              color: Colors.white,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  const Padding(
                    padding: EdgeInsets.only(left: 20, right: 20),
                    child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                      Text("10101",
                          style: TextStyle(
                              color: tenTenOnePurple, fontWeight: FontWeight.bold, fontSize: 40)),
                      Text("Beta\nDisclaimer.",
                          style: TextStyle(
                              color: Colors.black87, fontWeight: FontWeight.bold, fontSize: 40)),
                      SizedBox(height: 20),
                    ]),
                  ),
                  Expanded(
                    child: SingleChildScrollView(
                      child: Padding(
                        padding: const EdgeInsets.only(left: 20, right: 20),
                        child: Text("Important: The 10101 Wallet is still in its testing phase and is provided on an \"as is\" basis and may contain defects.\n\n".toUpperCase() +
                            "A primary purpose of this beta testing phase is to obtain feedback on performance and the identification of defects."
                                .toUpperCase() +
                            " Users are advised to safeguard important data, to use caution and not to rely in any way on the correct functioning or performance of the beta product."
                                .toUpperCase()),
                      ),
                    ),
                  ),
                  Container(
                    decoration: BoxDecoration(
                        color: tenTenOnePurple.withOpacity(0.05),
                        borderRadius: const BorderRadius.only(
                            topLeft: Radius.circular(10.0), topRight: Radius.circular(10.0))),
                    child: Padding(
                      padding: const EdgeInsets.only(left: 20, right: 20, bottom: 40),
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.end,
                        crossAxisAlignment: CrossAxisAlignment.end,
                        children: [
                          const SizedBox(height: 20),
                          Container(
                            padding: const EdgeInsets.all(15),
                            decoration: BoxDecoration(
                                color: tenTenOnePurple.shade300.withOpacity(0.2),
                                borderRadius: BorderRadius.circular(10.0)),
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                const Expanded(
                                  child: Text(
                                      "10101 is still being tested and may contain defects.",
                                      softWrap: true),
                                ),
                                TenTenOneSwitch(
                                    value: _betaDisclaimer,
                                    onChanged: (value) => setState(() => _betaDisclaimer = value)),
                              ],
                            ),
                          ),
                          const SizedBox(height: 10),
                          Container(
                            padding: const EdgeInsets.all(15),
                            decoration: BoxDecoration(
                                color: tenTenOnePurple.shade300.withOpacity(0.2),
                                borderRadius: BorderRadius.circular(10.0)),
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                const Expanded(
                                  child: Text(
                                      "If I lose my seed phrase and my device, my funds will be lost forever",
                                      softWrap: true),
                                ),
                                TenTenOneSwitch(
                                    value: _loseDisclaimer,
                                    onChanged: (value) => setState(() => _loseDisclaimer = value)),
                              ],
                            ),
                          ),
                          const SizedBox(height: 10),
                          Form(
                            key: _formKey,
                            child: TextFormField(
                              keyboardType: TextInputType.emailAddress,
                              initialValue: _email,
                              decoration: InputDecoration(
                                  border:
                                      OutlineInputBorder(borderRadius: BorderRadius.circular(10.0)),
                                  enabledBorder: OutlineInputBorder(
                                      borderRadius: BorderRadius.circular(10.0),
                                      borderSide: BorderSide(
                                          color: tenTenOnePurple.shade300.withOpacity(0.2))),
                                  filled: true,
                                  fillColor: tenTenOnePurple.shade300.withOpacity(0.2),
                                  labelText: 'Email (optional)',
                                  labelStyle: const TextStyle(color: Colors.black87, fontSize: 14),
                                  hintText: 'Let us know how to reach you'),
                              validator: (value) {
                                if (value == null || value.isEmpty) {
                                  return null;
                                }

                                if (!isEmailValid(value)) {
                                  return 'Please enter a valid email address';
                                }
                                return null;
                              },
                              onSaved: (value) {
                                _email = value ?? "";
                              },
                            ),
                          ),
                          const SizedBox(height: 25),
                          SizedBox(
                            width: MediaQuery.of(context).size.width * 0.9,
                            child: ElevatedButton(
                                onPressed: !(_betaDisclaimer && _loseDisclaimer)
                                    ? null
                                    : () {
                                        _formKey.currentState!.save();
                                        if (_formKey.currentState != null &&
                                            _formKey.currentState!.validate()) {
                                          GoRouter.of(context)
                                              .go(LoadingScreen.route, extra: setupWallet());
                                        }
                                      },
                                style: ButtonStyle(
                                    padding: MaterialStateProperty.all<EdgeInsets>(
                                        const EdgeInsets.all(15)),
                                    backgroundColor: MaterialStateProperty.resolveWith((states) {
                                      if (states.contains(MaterialState.disabled)) {
                                        return tenTenOnePurple.shade100;
                                      } else {
                                        return tenTenOnePurple;
                                      }
                                    }),
                                    shape: MaterialStateProperty.resolveWith((states) {
                                      if (states.contains(MaterialState.disabled)) {
                                        return RoundedRectangleBorder(
                                          borderRadius: BorderRadius.circular(30.0),
                                          side: BorderSide(color: tenTenOnePurple.shade100),
                                        );
                                      } else {
                                        return RoundedRectangleBorder(
                                          borderRadius: BorderRadius.circular(30.0),
                                          side: const BorderSide(color: tenTenOnePurple),
                                        );
                                      }
                                    })),
                                child: const Text(
                                  "Continue",
                                  style: TextStyle(fontSize: 18, color: Colors.white),
                                )),
                          ),
                        ],
                      ),
                    ),
                  )
                ],
              ),
            ),
          ),
        ));
  }

  Future<void> setupWallet() async {
    var seedPath = await getSeedFilePath();
    await Preferences.instance.setEmailAddress(_email);
    logger.i("Successfully stored the email address $_email .");
    await api.initNewMnemonic(targetSeedFilePath: seedPath);
    await api.registerBeta(email: _email);
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
