import 'package:flutter/material.dart';
import 'package:get_10101/auth/auth_service.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/text_input_field.dart';
import 'package:get_10101/common/version_service.dart';
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.center,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        const Spacer(),
        Image.asset('assets/10101_logo_icon.png', width: 350, height: 350),
        SizedBox(
            width: 500,
            height: 150,
            child: Container(
              padding: const EdgeInsets.all(18),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(8),
                color: Colors.grey[100],
              ),
              child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: [
                    TextInputField(
                      value: "",
                      label: "Password",
                      obscureText: true,
                      onSubmitted: (value) => value.isNotEmpty ? signIn(context, value) : (),
                      onChanged: (value) => setState(() => _password = value),
                    ),
                    const SizedBox(height: 20),
                    ElevatedButton(
                        onPressed: _password.isEmpty ? null : () => signIn(context, _password),
                        child: Container(
                            padding: const EdgeInsets.all(10),
                            child: const Text(
                              "Sign in",
                              style: TextStyle(fontSize: 16),
                            )))
                  ]),
            )),
        const Spacer(),
        FutureBuilder(
            future: versionService.fetchVersion(),
            builder: (context, snapshot) {
              if (snapshot.hasData) {
                return Text("Version: ${snapshot.data!.version}");
              } else {
                return const Text("Version: n/a");
              }
            }),
      ],
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
