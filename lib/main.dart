import 'package:flutter/material.dart';

import 'package:tentenone/ffi.io.dart';

void main() {
  runApp(const TenTenOneApp());
}

class TenTenOneApp extends StatelessWidget {
  const TenTenOneApp({super.key});

  // This widget is the root of your application.
  @override
  Widget build(BuildContext context) {
    return MaterialApp(
        title: '10101',
        theme: ThemeData(primarySwatch: Colors.blue),
        routes: {
          "/": (context) => Scaffold(
              body: Container(
                  color: Colors.white,
                  child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Center(child: Text(api.helloWorld())),
                      ])))
        });
  }
}
