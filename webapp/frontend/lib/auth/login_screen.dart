import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get_10101/services/auth_service.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/text_input_field.dart';
import 'package:get_10101/services/version_service.dart';
import 'package:get_10101/trade/trade_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class LoginScreen extends StatefulWidget {
  static const route = "/login";

  const LoginScreen({super.key});

  @override
  State<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends State<LoginScreen> {
  String _password = "";

  @override
  Widget build(BuildContext context) {
    final versionService = context.read<VersionService>();

    const minWidth = 350.0;
    final screenWidth = MediaQuery.of(context).size.width;

    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        return SingleChildScrollView(
          child: ConstrainedBox(
            constraints: constraints.copyWith(
              minHeight: constraints.maxHeight,
              maxHeight: double.infinity,
            ),
            child: IntrinsicHeight(
              child: Column(
                children: <Widget>[
                  const Spacer(),
                  ConstrainedBox(
                    constraints: BoxConstraints(
                      maxWidth: max(screenWidth, minWidth),
                    ),
                    child: SizedBox(
                        height: 350, width: 350, child: SvgPicture.asset('assets/10101_logo.svg')),
                  ),
                  Center(
                    child: SizedBox(
                        width: 500,
                        child: Container(
                          padding: const EdgeInsets.all(18),
                          decoration: BoxDecoration(
                            borderRadius: BorderRadius.circular(8),
                          ),
                          child: Column(
                              mainAxisAlignment: MainAxisAlignment.center,
                              crossAxisAlignment: CrossAxisAlignment.center,
                              children: [
                                TextInputField(
                                  value: "",
                                  label: "Password",
                                  obscureText: true,
                                  onSubmitted: (value) =>
                                      value.isNotEmpty ? signIn(context, value) : (),
                                  onChanged: (value) => setState(() => _password = value),
                                ),
                                const SizedBox(height: 20),
                                ElevatedButton(
                                    onPressed:
                                        _password.isEmpty ? null : () => signIn(context, _password),
                                    child: Container(
                                        padding: const EdgeInsets.all(10),
                                        child: const Text(
                                          "Sign in",
                                          style: TextStyle(fontSize: 16),
                                        )))
                              ]),
                        )),
                  ),
                  Expanded(
                    child: Align(
                      alignment: Alignment.bottomCenter,
                      child: Container(
                        width: double.infinity,
                        padding: const EdgeInsets.all(12.0),
                        child: FutureBuilder(
                            future: versionService.fetchVersion(),
                            builder: (context, snapshot) {
                              if (snapshot.hasData) {
                                return Text("Version: ${snapshot.data!.version}",
                                    textAlign: TextAlign.center);
                              } else {
                                return const Text("Version: n/a", textAlign: TextAlign.center);
                              }
                            }),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

void signIn(BuildContext context, String password) {
  final authService = context.read<AuthService>();
  authService
      .signIn(password)
      .then((value) => GoRouter.of(context).go(TradeScreen.route))
      .catchError((error) {
    final messenger = ScaffoldMessenger.of(context);
    showSnackBar(messenger, error?.toString() ?? "Failed to login!");
  });
}
